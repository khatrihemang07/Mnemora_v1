<!--
  ListNameField: an inline field for a List's name, used to create a List from the sidebar's + and to
  rename one (sidebar menu, the List page's title). Enter saves through `onsubmit`; if Rust refuses
  (a taken name, too long) the reason shows under the field and the text stays. Esc or clicking
  away cancels. The field takes focus on mount and selects the text, so a rename is one keystroke.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { IpcError } from '../../lib/ipc/unwrap';
  import Input from '../../lib/ui/Input.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { MAX_LIST_NAME } from './lists';

  interface Props {
    value?: string;
    label: string;
    placeholder?: string;
    /** Saves the name; throws to refuse it. */
    onsubmit: (name: string) => Promise<void>;
    oncancel: () => void;
    /** The reason floats over what is below (for the top bar) instead of pushing it down. */
    floating?: boolean;
    class?: string;
  }
  let {
    value: initial = '',
    label,
    placeholder,
    onsubmit,
    oncancel,
    floating = false,
    class: cls = '',
  }: Props = $props();

  let value = $state('');
  let problem = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let saving = false;

  onMount(() => {
    value = initial;
    input?.focus();
    input?.select();
  });

  async function submit() {
    if (saving) return;
    const name = value.trim();
    if (!name) {
      oncancel();
      return;
    }
    if (name === initial) {
      oncancel();
      return;
    }
    saving = true;
    try {
      await onsubmit(name);
    } catch (e) {
      if (e instanceof IpcError && e.code === 'validation') problem = e.message;
      else notifyError("Couldn't save the List");
      input?.focus();
    } finally {
      saving = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      void submit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      oncancel();
    }
  }
</script>

<div class="relative min-w-0 {cls}">
  <Input
    bare
    bind:ref={input}
    bind:value
    {label}
    {placeholder}
    maxlength={MAX_LIST_NAME}
    autocomplete="off"
    invalid={problem !== null}
    class="border-b py-px leading-[1.45] {problem ? 'border-danger' : 'border-accent'}"
    oninput={() => (problem = null)}
    {onkeydown}
    onblur={() => {
      if (!saving) oncancel();
    }}
  />
  {#if problem}
    <p
      role="alert"
      class="text-xs text-danger {floating
        ? 'absolute top-full left-0 z-10 mt-1.5 rounded-sm bg-pop px-2 py-1 whitespace-nowrap shadow-pop'
        : 'pt-1'}"
    >
      {problem}
    </p>
  {/if}
</div>
