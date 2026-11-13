use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum Delivery {
    Pasted,
    /// Only copied; `message` tells the user what to do.
    Copied {
        message: String,
    },
}

impl Delivery {
    fn copied(message: &str) -> Self {
        Self::Copied {
            message: message.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Field {
    pub role: String,
    pub description: String,
    pub dom_classes: String,
}

// The composer is a ProseMirror editor labelled "Prompt"; the terminal panel is xterm.
pub fn is_message_box(field: &Field) -> bool {
    let has_class = |name: &str| field.dom_classes.split_whitespace().any(|c| c == name);
    field.role == "AXTextArea"
        && !has_class("xterm-helper-textarea")
        && (has_class("ProseMirror") || field.description == "Prompt")
}

/// `closeness[i]`: depth of the deepest ancestor box `i` shares with the focused
/// element (deeper = same pane). Returns `None` rather than guess.
pub fn choose_message_box(focused: Option<usize>, closeness: &[Option<usize>]) -> Option<usize> {
    if focused.is_some() {
        return focused;
    }
    match closeness.len() {
        0 => None,
        1 => Some(0),
        _ => {
            let best = closeness.iter().flatten().max()?;
            let mut nearest = closeness
                .iter()
                .enumerate()
                .filter(|(_, c)| c.as_ref() == Some(best));
            let (index, _) = nearest.next()?;
            nearest.next().is_none().then_some(index)
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{deliver, has_permission, open_permission_settings};

#[cfg(not(target_os = "macos"))]
pub fn has_permission() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn open_permission_settings() {}

#[cfg(not(target_os = "macos"))]
pub fn deliver() -> Delivery {
    Delivery::copied("Copied. Paste it into Claude with Ctrl+V.")
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use accessibility_sys::{
        kAXChildrenAttribute, kAXDescriptionAttribute, kAXErrorSuccess, kAXFocusedAttribute,
        kAXFocusedUIElementAttribute, kAXFocusedWindowAttribute, kAXMainWindowAttribute,
        kAXMenuBarAttribute, kAXParentAttribute, kAXPressAction, kAXRoleAttribute,
        kAXTrustedCheckOptionPrompt, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
        AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementGetTypeID,
        AXUIElementPerformAction, AXUIElementRef, AXUIElementSetAttributeValue,
    };
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFEqual, CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    use objc2_foundation::NSString;

    use super::{choose_message_box, is_message_box, Delivery, Field};

    const BUNDLE_ID: &str = "com.anthropic.claudefordesktop";
    // Electron builds its AX tree lazily after AXManualAccessibility is set.
    const TREE_TIMEOUT: Duration = Duration::from_millis(1500);
    const FOCUS_TIMEOUT: Duration = Duration::from_millis(600);
    const ACTIVATE_TIMEOUT: Duration = Duration::from_millis(1000);
    const MAX_NODES: usize = 60_000;
    static PROMPTED: AtomicBool = AtomicBool::new(false);

    pub fn deliver() -> Delivery {
        let Some(claude) = running_claude() else {
            return Delivery::copied("Copied. Claude isn't open, so paste it there with ⌘V.");
        };
        if !is_trusted() {
            return Delivery::copied(
                "Copied. To paste into Claude automatically, allow Hush under System Settings → Privacy & Security → Accessibility.",
            );
        }

        let app = Element::application(claude.processIdentifier());
        app.set_bool("AXManualAccessibility", true);
        claude.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        wait_until(ACTIVATE_TIMEOUT, || claude.isActive());

        let Some(target) = poll(TREE_TIMEOUT, || find_message_box(&app)) else {
            return Delivery::copied(
                "Copied. Couldn't tell which Claude message box to use, so paste it with ⌘V.",
            );
        };
        if app.element(kAXFocusedUIElementAttribute).as_ref() != Some(&target) {
            target.set_bool(kAXFocusedAttribute, true);
            let focused = wait_until(FOCUS_TIMEOUT, || {
                app.element(kAXFocusedUIElementAttribute).as_ref() == Some(&target)
            });
            if !focused {
                return Delivery::copied(
                    "Copied. Couldn't focus Claude's message box, so paste it with ⌘V.",
                );
            }
        }
        match find_paste_item(&app) {
            Some(paste) if paste.press() => Delivery::Pasted,
            _ => Delivery::copied("Copied. Paste it into Claude's message box with ⌘V."),
        }
    }

    fn running_claude() -> Option<Retained<NSRunningApplication>> {
        let id = NSString::from_str(BUNDLE_ID);
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&id).firstObject()
    }

    pub fn has_permission() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn open_permission_settings() {
        let url = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
        let _ = std::process::Command::new("open").arg(url).status();
    }

    fn is_trusted() -> bool {
        if unsafe { AXIsProcessTrusted() } {
            return true;
        }
        if !PROMPTED.swap(true, Ordering::Relaxed) {
            let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
            let options = CFDictionary::from_CFType_pairs(&[(
                key.as_CFType(),
                CFBoolean::true_value().as_CFType(),
            )]);
            unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
        }
        false
    }

    fn find_message_box(app: &Element) -> Option<Element> {
        let window = app
            .element(kAXFocusedWindowAttribute)
            .or_else(|| app.element(kAXMainWindowAttribute))?;
        let mut boxes = Vec::new();
        let mut visited = 0;
        window.walk(&mut visited, &mut |el| {
            if is_message_box(&el.field()) {
                boxes.push(el.clone());
            }
        });
        if boxes.is_empty() {
            return None;
        }
        let focused = app.element(kAXFocusedUIElementAttribute);
        let focused_index = focused
            .as_ref()
            .and_then(|f| boxes.iter().position(|b| b == f));
        let focus_path = focused.as_ref().map(Element::ancestors);
        let closeness: Vec<Option<usize>> = boxes
            .iter()
            .map(|b| {
                focus_path
                    .as_ref()
                    .map(|path| shared_depth(path, &b.ancestors()))
            })
            .collect();
        choose_message_box(focused_index, &closeness).map(|i| boxes.swap_remove(i))
    }

    // Matched by shortcut rather than title so it works in any language.
    fn find_paste_item(app: &Element) -> Option<Element> {
        let bar = app.element(kAXMenuBarAttribute)?;
        let mut found = None;
        let mut visited = 0;
        bar.walk(&mut visited, &mut |el| {
            if found.is_none()
                && el.string("AXMenuItemCmdChar") == "V"
                && el.number("AXMenuItemCmdModifiers") == Some(0)
            {
                found = Some(el.clone());
            }
        });
        found
    }

    fn shared_depth(a: &[Element], b: &[Element]) -> usize {
        a.iter()
            .rev()
            .zip(b.iter().rev())
            .take_while(|(x, y)| x == y)
            .count()
    }

    fn poll<T>(timeout: Duration, mut attempt: impl FnMut() -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(value) = attempt() {
                return Some(value);
            }
            if Instant::now() >= deadline {
                return None;
            }
            sleep(Duration::from_millis(60));
        }
    }

    fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
        poll(timeout, || done().then_some(())).is_some()
    }

    struct Element(AXUIElementRef);

    // SAFETY: AXUIElementRef is a Core Foundation object; Element holds one retain on it
    // and the Accessibility API may be used from any thread.
    unsafe impl Send for Element {}

    impl Element {
        fn application(pid: i32) -> Self {
            Self(unsafe { AXUIElementCreateApplication(pid) })
        }

        fn from_borrowed(raw: CFTypeRef) -> Option<Self> {
            if raw.is_null()
                || unsafe { core_foundation::base::CFGetTypeID(raw) }
                    != unsafe { AXUIElementGetTypeID() }
            {
                return None;
            }
            unsafe { CFRetain(raw) };
            Some(Self(raw as AXUIElementRef))
        }

        fn attribute(&self, name: &str) -> Option<CFType> {
            let name = CFString::new(name);
            let mut value: CFTypeRef = ptr::null();
            let err = unsafe {
                AXUIElementCopyAttributeValue(self.0, name.as_concrete_TypeRef(), &mut value)
            };
            (err == kAXErrorSuccess && !value.is_null())
                .then(|| unsafe { CFType::wrap_under_create_rule(value) })
        }

        fn element(&self, name: &str) -> Option<Element> {
            self.attribute(name)
                .and_then(|value| Self::from_borrowed(value.as_CFTypeRef()))
        }

        fn children(&self) -> Vec<Element> {
            let Some(array) = self
                .attribute(kAXChildrenAttribute)
                .and_then(|v| v.downcast::<CFArray>())
            else {
                return Vec::new();
            };
            array
                .iter()
                .filter_map(|item| Self::from_borrowed(*item))
                .collect()
        }

        // String lists (e.g. AXDOMClassList) are joined with spaces.
        fn string(&self, name: &str) -> String {
            let Some(value) = self.attribute(name) else {
                return String::new();
            };
            if let Some(text) = value.downcast::<CFString>() {
                return text.to_string();
            }
            value
                .downcast::<CFArray>()
                .map(|list| {
                    list.iter()
                        .filter(|item| !item.is_null())
                        .filter_map(|item| {
                            unsafe { CFType::wrap_under_get_rule(*item) }.downcast::<CFString>()
                        })
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        }

        fn number(&self, name: &str) -> Option<i64> {
            self.attribute(name)?.downcast::<CFNumber>()?.to_i64()
        }

        fn field(&self) -> Field {
            Field {
                role: self.string(kAXRoleAttribute),
                description: self.string(kAXDescriptionAttribute),
                dom_classes: self.string("AXDOMClassList"),
            }
        }

        fn set_bool(&self, name: &str, value: bool) {
            let name = CFString::new(name);
            let value = CFBoolean::from(value);
            unsafe {
                AXUIElementSetAttributeValue(
                    self.0,
                    name.as_concrete_TypeRef(),
                    value.as_CFTypeRef(),
                )
            };
        }

        fn press(&self) -> bool {
            let action = CFString::new(kAXPressAction);
            unsafe {
                AXUIElementPerformAction(self.0, action.as_concrete_TypeRef()) == kAXErrorSuccess
            }
        }

        fn ancestors(&self) -> Vec<Element> {
            let mut path = vec![self.clone()];
            while path.len() < 400 {
                match path.last().and_then(|el| el.element(kAXParentAttribute)) {
                    Some(parent) => path.push(parent),
                    None => break,
                }
            }
            path
        }

        fn walk(&self, visited: &mut usize, visit: &mut dyn FnMut(&Element)) {
            *visited += 1;
            if *visited > MAX_NODES {
                return;
            }
            visit(self);
            for child in self.children() {
                child.walk(visited, visit);
            }
        }
    }

    impl Clone for Element {
        fn clone(&self) -> Self {
            unsafe { CFRetain(self.0 as CFTypeRef) };
            Self(self.0)
        }
    }

    impl PartialEq for Element {
        fn eq(&self, other: &Self) -> bool {
            unsafe { CFEqual(self.0 as CFTypeRef, other.0 as CFTypeRef) != 0 }
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0 as CFTypeRef) };
        }
    }

    // Read-only. Run with: cargo test claude_dry_run -- --ignored --nocapture
    #[test]
    #[ignore = "needs the Claude app and the Accessibility permission"]
    fn claude_dry_run() {
        let claude = running_claude().expect("Claude is running");
        assert!(
            unsafe { AXIsProcessTrusted() },
            "this process needs the Accessibility permission"
        );
        let app = Element::application(claude.processIdentifier());
        app.set_bool("AXManualAccessibility", true);
        let target = poll(TREE_TIMEOUT, || find_message_box(&app)).expect("a message box");
        let field = target.field();
        println!(
            "would paste into: role={} description={:?} classes={:?}",
            field.role, field.description, field.dom_classes
        );
        assert!(is_message_box(&field));
        let paste = find_paste_item(&app).expect("Claude's Paste menu item");
        println!("paste menu item: {:?}", paste.string("AXTitle"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(role: &str, description: &str, classes: &str) -> Field {
        Field {
            role: role.into(),
            description: description.into(),
            dom_classes: classes.into(),
        }
    }

    #[test]
    fn recognizes_the_message_box_but_never_the_terminal() {
        assert!(is_message_box(&field(
            "AXTextArea",
            "Prompt",
            "tiptap ProseMirror"
        )));
        assert!(is_message_box(&field("AXTextArea", "", "ProseMirror")));
        assert!(!is_message_box(&field(
            "AXTextField",
            "Terminal input",
            "xterm-helper-textarea"
        )));
        assert!(!is_message_box(&field(
            "AXTextArea",
            "Prompt",
            "xterm-helper-textarea"
        )));
        assert!(!is_message_box(&field("AXTextField", "Search", "")));
    }

    #[test]
    fn uses_the_focused_or_only_message_box() {
        assert_eq!(choose_message_box(Some(1), &[Some(2), Some(5)]), Some(1));
        assert_eq!(choose_message_box(None, &[None]), Some(0));
        assert_eq!(choose_message_box(None, &[]), None);
    }

    #[test]
    fn picks_the_box_in_the_same_pane_as_the_focus() {
        assert_eq!(choose_message_box(None, &[Some(4), Some(9)]), Some(1));
    }

    #[test]
    fn refuses_to_guess_between_equally_likely_boxes() {
        assert_eq!(choose_message_box(None, &[Some(4), Some(4)]), None);
        assert_eq!(choose_message_box(None, &[None, None]), None);
    }
}
