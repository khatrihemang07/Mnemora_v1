<!--
  MarkdownInline: one line of text with inline Markdown shown (bold, italic, code, links, bare URLs),
  for a Task title or a comment. It uses the Journal's parser, so block syntax such as "#" or "- "
  stays as the text it is, and draws Svelte elements from plain data, never raw HTML. Links open in
  the system browser without activating whatever contains them (a row).
-->
<script lang="ts">
  import { openExternal } from '../open-external';
  import type { Inline } from './markdown/ast';
  import { parseInlineMarkdown } from './markdown/inline';

  interface Props {
    source: string;
    class?: string;
  }
  let { source, class: cls = '' }: Props = $props();

  const nodes = $derived(parseInlineMarkdown(source));

  function follow(e: MouseEvent, href: string) {
    e.preventDefault();
    e.stopPropagation();
    void openExternal(href);
  }
</script>

{#snippet inline(list: Inline[])}
  {#each list as n, i (i)}
    {#if n.t === 'text'}{n.s}{:else if n.t === 'code'}<code class="md-code">{n.s}</code>{:else if n.t === 'strong'}<strong>{@render inline(n.c)}</strong>{:else if n.t === 'em'}<em>{@render inline(n.c)}</em>{:else if n.t === 'link'}<a class="md-link" href={n.href} onclick={(e) => follow(e, n.href)}>{@render inline(n.c)}</a>{:else}{@render inline(n.c)}{/if}
  {/each}
{/snippet}

<span class={cls}>{@render inline(nodes)}</span>
