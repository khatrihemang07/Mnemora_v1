import { commands, events, type PairingView } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import { urlParam } from '../../lib/router/url-param';
import { notify } from '../../lib/ui/toast';
import { parseAddress } from './address';
import { problem } from './pair-view';
import { refreshSync } from './queries';

export type PairMode = 'show' | 'enter' | 'address';

/** The history entry an open sheet adds (`/sync?pair=show`), so browser Back closes it. */
const PARAM = 'pair';

/**
 * The pairing sheet's state. Rust owns the phases (`view` is whatever it last said); this adds only
 * what the sheet itself remembers: whether it is open, which way round it was started, the digits
 * typed so far, and when the view arrived (the countdown counts from there).
 */
class Pairing {
  open = $state(false);
  mode = $state<PairMode>('show');
  view = $state<PairingView>({ state: 'idle' });
  /** Digits typed in the boxes. */
  code = $state('');
  /** The other device's address, for "Pair by address…". */
  address = $state('');
  /** What is wrong with the address typed, before anything is sent. */
  addressError = $state<string | null>(null);
  /** A command that failed outright (Sync not ready); pairing failures arrive in `view`. */
  commandError = $state<unknown>(null);
  /** The phone is on mobile data with "Allow over mobile data" off: the sheet says so instead of showing a code. */
  pausedOnMobile = $state(false);
  arrivedAt = $state(Date.now());
  /** Whether opening pushed the history entry that closing should step back over. */
  #pushed = false;

  /** Called once the sheet is mounted; returns what stops listening. */
  async listen(): Promise<() => void> {
    const stopView = await events.syncPairingEvent.listen((e) => this.receive(e.payload));
    // Rust announces the device status when the phone changes network (Wi-Fi, mobile data).
    const stopPeers = await events.syncPeersEvent.listen(() => void this.refreshPause());
    this.receive(await commands.syncPairState());
    return () => {
      stopView();
      stopPeers();
    };
  }

  private receive(view: PairingView) {
    this.view = view;
    this.arrivedAt = Date.now();
    // A code that cannot work any more is not worth keeping in the boxes.
    if (view.state === 'failed' && problem(view.error, this.mode !== 'show').deadCode) {
      this.code = '';
    }
    if (view.state === 'paired') void refreshSync();
  }

  /** "Pair a device" shows a code; "Enter a code" types one. */
  async start(mode: PairMode) {
    if (!this.open) {
      this.#pushed = urlParam.get(PARAM) === null;
      urlParam.set(PARAM, mode, !this.#pushed);
    }
    this.open = true;
    this.mode = mode;
    this.code = '';
    this.addressError = null;
    this.commandError = null;
    this.view = { state: 'idle' };
    this.pausedOnMobile = await this.onMobileData();
    if (this.pausedOnMobile) return;
    if (mode === 'show') await this.run(() => unwrap(commands.syncPairShow()));
  }

  /** The phone switched network with the sheet open: the sheet explains, or starts again, to match. */
  async refreshPause() {
    if (!this.open) return;
    const paused = await this.onMobileData();
    if (!this.open) return;
    const retry =
      !paused &&
      this.mode === 'show' &&
      this.view.state === 'idle' &&
      (this.pausedOnMobile || this.commandError !== null);
    if (paused === this.pausedOnMobile && !retry) return;
    this.pausedOnMobile = paused;
    if (paused) {
      // The network is gone with whatever it was showing.
      this.view = { state: 'idle' };
      return;
    }
    this.commandError = null;
    if (this.mode === 'show') await this.run(() => unwrap(commands.syncPairShow()));
  }

  /** Asked before anything is started, so the sheet can say why it will not pair. */
  private async onMobileData(): Promise<boolean> {
    try {
      return (await unwrap(commands.syncDevices())).this_device.paused_on_mobile_data;
    } catch {
      // The command itself will report what is wrong.
      return false;
    }
  }

  /** Switches the open sheet between showing a code and typing one. */
  async switchTo(mode: PairMode) {
    await commands.syncPairCancel();
    await this.start(mode);
  }

  async newCode() {
    this.commandError = null;
    await this.run(() => unwrap(commands.syncPairNewCode()));
  }

  async submit() {
    if (this.code.length !== 8) return;
    this.commandError = null;
    await this.run(() => unwrap(commands.syncPairEnter(this.code)));
  }

  /** Says what is wrong with the typed address, if anything; true when it can be dialled. An empty field stays quiet. */
  checkAddress(): boolean {
    const parsed = parseAddress(this.address);
    this.addressError = !parsed.ok && this.address.trim() !== '' ? parsed.message : null;
    return parsed.ok;
  }

  /** Pairs with the device at `address`, which shows `code`. */
  async submitAddress() {
    // The address is judged first, so a bad one says so whatever the boxes hold, and nothing is dialled.
    if (!this.checkAddress() || this.code.length !== 8) return;
    this.commandError = null;
    await this.run(() => unwrap(commands.syncPairEnterAt(this.address, this.code)));
  }

  /** The address was edited: the failure about the old one no longer applies. */
  editAddress(value: string) {
    this.address = value;
    this.addressError = null;
    this.forgetFailure();
  }

  /** The code was edited: the failure about the old one no longer applies. */
  editCode(value: string) {
    if (value === this.code) return;
    this.code = value;
    this.forgetFailure();
  }

  private forgetFailure() {
    if (this.view.state === 'failed') this.view = { state: 'idle' };
  }

  async decide(allow: boolean) {
    const asked = this.view.state === 'asking' ? this.view.name : null;
    // Deny ends the pairing (the other device is told it was declined), so the sheet has nothing
    // left to show: it closes at once, before the new view arrives and shows an empty step.
    if (!allow) this.shut();
    await this.run(() => unwrap(commands.syncPairDecide(allow)));
    if (!allow) {
      if (this.commandError) this.open = true;
      else notify(asked ? `Declined ${asked}` : 'Declined');
    }
  }

  /**
   * Merge and sync, or Cancel, on the merge preview. Cancel tells the other device and leaves both
   * databases as they were, so the sheet has nothing left to show.
   */
  async merge(merge: boolean) {
    if (!merge) this.shut();
    await this.run(() => unwrap(commands.syncPairMerge(merge)));
  }

  /** Closes the sheet; whatever was going on stops, and whoever was asked is told. */
  async close(stepBack = true) {
    if (stepBack) this.shut();
    else {
      // The page is being left: its history stays as it is.
      this.open = false;
      this.#pushed = false;
    }
    this.commandError = null;
    await commands.syncPairCancel();
    // The list behind the sheet may have missed a pairing made while it was open.
    await refreshSync();
  }

  /** The sheet is gone from the screen: its history entry is stepped back over. */
  private shut() {
    this.open = false;
    if (!this.#pushed) return;
    this.#pushed = false;
    history.back();
  }

  /** Back removed the entry while the sheet was open: close it (and end the pairing) without stepping back again. */
  backed() {
    if (!this.open || !this.#pushed) return;
    this.#pushed = false;
    void this.close(false);
  }

  /**
   * A `?pair=` left in the address with no sheet behind it (a reload, Forward onto the entry, Back onto
   * the entry of a page that was left): nothing is pairing, so the parameter is dropped in place.
   */
  dropStale() {
    if (!this.open && urlParam.get(PARAM) !== null) urlParam.delete(PARAM, true);
  }

  private async run(call: () => Promise<unknown>) {
    try {
      await call();
    } catch (e) {
      this.commandError = e;
    }
  }
}

export const pairing = new Pairing();
