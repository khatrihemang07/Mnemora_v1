/**
 * The fold rule for a Module's nested sidebar tree (Todo's views and Lists):
 * - entering the Module unfolds the tree and leaving it folds it;
 * - while inside, clicking the Module's row or its chevron flips it;
 * - that flip lasts only until the next enter or leave, when the automatic rule applies again.
 * The `open` flag is the one thing remembered on this device.
 */
export interface TreeFold {
  /** The tree is unfolded. */
  open: boolean;
  /** The current page belongs to the Module. */
  inside: boolean;
}

/** The page changed: entering unfolds, leaving folds, staying keeps whatever the user chose. */
export function onNavigate(fold: TreeFold, inside: boolean): TreeFold {
  if (inside === fold.inside) return fold;
  return { open: inside, inside };
}

/** The chevron: flips the tree wherever you are. */
export const toggle = (fold: TreeFold): TreeFold => ({ ...fold, open: !fold.open });

/** What a click on the Module's row does: inside it flips the tree, outside it goes there. */
export const rowClick = (fold: TreeFold): 'toggle' | 'enter' => (fold.inside ? 'toggle' : 'enter');

/** The fold when the app starts: a saved choice wins; with none, the tree is open inside the Module. */
export const initialOpen = (saved: boolean | null, inside: boolean): boolean => saved ?? inside;
