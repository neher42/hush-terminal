//! The terminals in one tab, split side by side or top and bottom.

use gpui::{div, prelude::*, rgb, AnyElement, Axis, Entity, Subscription};

use crate::colors::Palette;
use crate::view::TerminalView;

pub enum Pane {
    Terminal {
        view: Entity<TerminalView>,
        _events: Subscription,
    },
    /// Children share the space equally. A split never holds a split of its own
    /// axis; those children are part of it directly.
    Split { axis: Axis, children: Vec<Pane> },
}

impl Pane {
    pub fn terminal(view: Entity<TerminalView>, events: Subscription) -> Self {
        Pane::Terminal {
            view,
            _events: events,
        }
    }

    fn is(&self, target: &Entity<TerminalView>) -> bool {
        matches!(self, Pane::Terminal { view, .. } if view == target)
    }

    /// Every terminal, left to right and top to bottom.
    pub fn views(&self) -> Vec<Entity<TerminalView>> {
        let mut views = Vec::new();
        self.collect(&mut views);
        views
    }

    fn collect(&self, views: &mut Vec<Entity<TerminalView>>) {
        match self {
            Pane::Terminal { view, .. } => views.push(view.clone()),
            Pane::Split { children, .. } => children.iter().for_each(|c| c.collect(views)),
        }
    }

    pub fn contains(&self, target: &Entity<TerminalView>) -> bool {
        match self {
            Pane::Terminal { view, .. } => view == target,
            Pane::Split { children, .. } => children.iter().any(|c| c.contains(target)),
        }
    }

    /// Puts `new` right of (or below) `target`.
    pub fn split(&mut self, target: &Entity<TerminalView>, axis: Axis, new: Pane) {
        self.insert(target, axis, &mut Some(new));
    }

    fn insert(&mut self, target: &Entity<TerminalView>, axis: Axis, new: &mut Option<Pane>) {
        match self {
            Pane::Terminal { view, .. } => {
                if view == target {
                    let old = std::mem::replace(
                        self,
                        Pane::Split {
                            axis,
                            children: Vec::new(),
                        },
                    );
                    if let (Pane::Split { children, .. }, Some(new)) = (self, new.take()) {
                        children.extend([old, new]);
                    }
                }
            }
            Pane::Split {
                axis: own,
                children,
            } => {
                if *own == axis {
                    if let Some(i) = children.iter().position(|c| c.is(target)) {
                        children.insert(i + 1, new.take().expect("inserted once"));
                        return;
                    }
                }
                for child in children {
                    child.insert(target, axis, new);
                    if new.is_none() {
                        return;
                    }
                }
            }
        }
    }

    /// Takes `target` out. Returns false when nothing is left.
    pub fn remove(&mut self, target: &Entity<TerminalView>) -> bool {
        let Pane::Split { axis, children } = self else {
            return !self.is(target);
        };
        children.retain_mut(|child| child.remove(target));
        // Children that became a split of the same axis join this one.
        let axis = *axis;
        let mut flat = Vec::with_capacity(children.len());
        for child in children.drain(..) {
            match child {
                Pane::Split {
                    axis: inner,
                    children: grandchildren,
                } if inner == axis => flat.extend(grandchildren),
                other => flat.push(other),
            }
        }
        match flat.len() {
            0 => false,
            1 => {
                *self = flat.pop().expect("one child");
                true
            }
            _ => {
                *children = flat;
                true
            }
        }
    }

    /// `wrap` adds what the window needs around each terminal (focus tracking).
    pub fn render(
        &self,
        palette: &Palette,
        wrap: &impl Fn(&Entity<TerminalView>) -> gpui::Div,
    ) -> AnyElement {
        match self {
            Pane::Terminal { view, .. } => wrap(view).child(view.clone()).into_any_element(),
            Pane::Split { axis, children } => div()
                .size_full()
                .flex()
                .when(*axis == Axis::Vertical, |el| el.flex_col())
                .children(children.iter().enumerate().map(|(i, child)| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .border_color(rgb(palette.line))
                        .when(i > 0 && *axis == Axis::Horizontal, |el| el.border_l_1())
                        .when(i > 0 && *axis == Axis::Vertical, |el| el.border_t_1())
                        .child(child.render(palette, wrap))
                }))
                .into_any_element(),
        }
    }
}
