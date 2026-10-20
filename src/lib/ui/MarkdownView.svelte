<!--
  MarkdownView: the read-only rendering of Markdown text. Symbols are hidden and formatting is shown;
  it is built from the same parser as MarkdownEditor and draws Svelte elements from plain data, never
  raw HTML. Links open in the system browser. Checklist items look like checkboxes; pass `ontoggle`
  to make them tickable (a later ticket wires it up).
-->
<script lang="ts">
  import Check from '~icons/lucide/check';
  import { openExternal } from '../open-external';
  import { type Block, type Inline, parseMarkdown } from './markdown/ast';

  interface Props {
    source: string;
    /** The body font: the display serif (default) or the UI sans. */
    font?: 'serif' | 'sans';
    /** Called with the item's 0-based line and its new state when a checkbox is activated. */
    ontoggle?: (line: number, checked: boolean) => void;
    class?: string;
  }
  let { source, font = 'serif', ontoggle, class: cls = '' }: Props = $props();

  const blocks = $derived(parseMarkdown(source));

  function follow(e: MouseEvent, href: string) {
    e.preventDefault();
    e.stopPropagation();
    void openExternal(href);
  }
</script>

{#snippet inline(nodes: Inline[])}
  {#each nodes as n, i (i)}
    {#if n.t === 'text'}{n.s}{:else if n.t === 'code'}<code class="md-code">{n.s}</code>{:else if n.t === 'strong'}<strong>{@render inline(n.c)}</strong>{:else if n.t === 'em'}<em>{@render inline(n.c)}</em>{:else if n.t === 'del'}<del>{@render inline(n.c)}</del>{:else if n.t === 'mark'}<mark class="md-mark">{@render inline(n.c)}</mark>{:else}<a class="md-link" href={n.href} onclick={(e) => follow(e, n.href)}>{@render inline(n.c)}</a>{/if}
  {/each}
{/snippet}

{#snippet body(list: Block[])}
  {#each list as b, i (i)}
    {#if b.t === 'p'}
      <p class="md-p">{@render inline(b.c)}</p>
    {:else if b.t === 'h'}
      <svelte:element this={`h${b.level}`} class="md-h md-h{b.level}">{@render inline(b.c)}</svelte:element>
    {:else if b.t === 'quote'}
      <blockquote class="md-quote">{@render body(b.c)}</blockquote>
    {:else if b.t === 'hr'}
      <hr class="md-hr" />
    {:else if b.t === 'code'}
      <pre class="md-pre">{b.s}</pre>
    {:else}
      <svelte:element
        this={b.ordered ? 'ol' : 'ul'}
        class="md-list"
        class:md-tasks={b.items.some((it) => it.task)}
        start={b.ordered ? b.start : undefined}
      >
        {#each b.items as item, j (j)}
          {#if item.task}
            {@const task = item.task}
            <li class="md-item md-task" class:md-task-done={task.checked}>
              <span
                class="md-cb"
                role="checkbox"
                aria-checked={task.checked}
                aria-readonly={!ontoggle}
                tabindex={ontoggle ? 0 : undefined}
                data-checked={task.checked || undefined}
                onclick={ontoggle
                  ? (e) => {
                      e.stopPropagation();
                      ontoggle(task.line, !task.checked);
                    }
                  : undefined}
                onkeydown={ontoggle
                  ? (e) => {
                      if (e.key === ' ' || e.key === 'Enter') {
                        e.preventDefault();
                        e.stopPropagation();
                        ontoggle(task.line, !task.checked);
                      }
                    }
                  : undefined}
              >
                {#if task.checked}<Check class="size-3" />{/if}
              </span>
              <div>{@render body(item.c)}</div>
            </li>
          {:else}
            <li class="md-item">{@render body(item.c)}</li>
          {/if}
        {/each}
      </svelte:element>
    {/if}
  {/each}
{/snippet}

<div class="md selectable {cls}" data-font={font}>
  {@render body(blocks)}
</div>
