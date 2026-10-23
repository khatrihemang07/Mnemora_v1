import { commands, events, type PairingView } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';
import { notify } from '../../lib/ui/toast';
import { parseAddress } from './address';
import { problem } from './pair-view';
import { refreshSync } from './queries';

export type PairMode = 'show' | 'enter' | 'address';

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
  arrivedAt = $state(Date.now());

  /** Called once the sheet is mounted; returns what stops listening. */
  async listen(): Promise<() => void> {
    const stop = await events.syncPairingEvent.listen((e) => this.receive(e.payload));
    this.receive(await commands.syncPairState());
    return stop;
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
    this.open = true;
    this.mode = mode;
    this.code = '';
    this.addressError = null;
    this.commandError = null;
    this.view = { state: 'idle' };
    if (mode === 'show') await this.run(() => unwrap(commands.syncPairShow()));
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

  /** Pairs with the device at `address`, which shows `code`. */
  async submitAddress() {
    if (this.code.length !== 8) return;
    const parsed = parseAddress(this.address);
    if (!parsed.ok) {
      this.addressError = parsed.message;
      return;
    }
    this.addressError = null;
    this.commandError = null;
    await this.run(() => unwrap(commands.syncPairEnterAt(this.address, this.code)));
  }

  async decide(allow: boolean) {
    const asked = this.view.state === 'asking' ? this.view.name : null;
    await this.run(() => unwrap(commands.syncPairDecide(allow)));
    // Deny ends the pairing (the other device is told it was declined), so the sheet has nothing
    // left to show; a short note says what happened.
    if (!allow) {
      this.open = false;
      if (!this.commandError) notify(asked ? `Declined ${asked}` : 'Declined');
    }
  }

  /** Closes the sheet; whatever was going on stops, and whoever was asked is told. */
  async close() {
    this.open = false;
    this.commandError = null;
    await commands.syncPairCancel();
    // The list behind the sheet may have missed a pairing made while it was open.
    await refreshSync();
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
