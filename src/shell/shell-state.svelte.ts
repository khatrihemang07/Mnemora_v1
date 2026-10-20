import { resetLayout, type SidebarLayout, toggleRail } from './sidebar-layout';

/** UI state shared by the sidebar, title bar, palette and Settings. */
class ShellState {
  paletteOpen = $state(false);
  layout = $state<SidebarLayout>(resetLayout());
  /** Layout from the device has been loaded; persisting before that would overwrite it. */
  layoutReady = $state(false);

  toggleSidebar() {
    this.layout = toggleRail(this.layout);
  }

  resetSidebar() {
    this.layout = resetLayout();
  }
}

export const shell = new ShellState();
