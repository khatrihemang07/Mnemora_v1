<!--
  MarkdownInline: one line of text with inline Markdown shown (bold, italic, code, links, bare URLs),
  for a Task title or a comment. It uses the Journal's parser, so block syntax such as "#" or "- "
  stays as the text it is, and draws Svelte elements from plain data, never raw HTML. Links open in
  the system browser without activating whatever contains them (a row). `marks` underline words
  with a dotted line and a tooltip (the words capture understood in a Task's title). `lit` tints the
  words a search matched (folded whole words), in the Module's hue.
-->
<script lang="ts">
  import { openExternal } from '../open-external';
  import type { Inline } from './markdown/ast';
  import { parseInlineMarkdown } from './markdown/inline';
  import { litPieces, markPieces, type Piece, type WordMark } from './mark-words';

  interface Props {
    source: string;
    class?: string;
    marks?: WordMark[];
    lit?: ReadonlySet<string>;
  }
  let { source, class: cls = '', marks = [], lit = new Set<string>() }: Props = $props();

  const nodes = $derived(parseInlineMarkdown(source));

  /** The pieces of every text node, with the marked words cut out, found by node. */
  const pieces = $derived.by(() => {
    const found = new Map<Inline, Piece[]>();
    if (marks.length === 0) return found;
    const texts: Inline[] = [];
    const walk = (list: Inline[]) => {
      for (const n of list) {
        if (n.t === 'text') texts.push(n);
        else if ('c' in n) walk(n.c);
      }
    };
    walk(nodes);
    const cut = markPieces(
      texts.map((n) => (n.t === 'text' ? n.s : '')),
      marks,
    );
    texts.forEach((n, i) => found.set(n, cut[i] ?? []));
    return found;
  });

  function follow(e: MouseEvent, href: string) {
    e.preventDefault();
    e.stopPropagation();
    void openExternal(href);
  }
</script>

{#snippet inline(list: Inline[])}
  {#each list as n, i (i)}
    {#if n.t === 'text' && lit.size > 0}{#each litPieces(n.s, lit) as p, j (j)}{#if p.lit}<mark class="rounded-[3px] bg-[color-mix(in_oklch,var(--hue-todo)_28%,transparent)] px-px text-inherit">{p.text}</mark>{:else}{p.text}{/if}{/each}{:else if n.t === 'text'}{#each pieces.get(n) ?? [{ text: n.s }] as p, j (j)}{#if p.title}<span class="cursor-help underline decoration-hue-todo decoration-dotted underline-offset-[3px]" title={p.title}>{p.text}</span>{:else}{p.text}{/if}{/each}{:else if n.t === 'code'}<code class="md-code">{n.s}</code>{:else if n.t === 'strong'}<strong>{@render inline(n.c)}</strong>{:else if n.t === 'em'}<em>{@render inline(n.c)}</em>{:else if n.t === 'link'}<a class="md-link" href={n.href} onclick={(e) => follow(e, n.href)}>{@render inline(n.c)}</a>{:else}{@render inline(n.c)}{/if}
  {/each}
{/snippet}

<span class={cls}>{@render inline(nodes)}</span>
