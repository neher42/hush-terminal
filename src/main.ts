import '@xterm/xterm/css/xterm.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/700.css';
import './styles.css';
import { App } from './app/App';
import { loadSettings } from './app/settings';
import { backend } from './backend';

function element<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`Missing #${id} in index.html`);
  return el as T;
}

async function main(): Promise<void> {
  // The terminal measures its font once at startup, so the font must be ready first.
  await Promise.all([document.fonts.load('13px "JetBrains Mono"'), document.fonts.load('bold 13px "JetBrains Mono"')]);
  const [home, settings] = await Promise.all([backend.homeDir(), loadSettings()]);
  const app = new App(
    {
      app: element('app'),
      tabs: element('tabs'),
      sidebar: element('sidebar'),
      terminalPane: element('terminal-pane'),
      toggleSidebar: element('toggle-sidebar'),
      newTab: element('new-tab'),
      copyCwd: element('copy-cwd'),
    },
    home,
    settings,
  );
  await app.start();
}

main().catch((err: unknown) => {
  console.error(err);
  document.body.textContent = `Hush couldn't start: ${String(err)}`;
});
