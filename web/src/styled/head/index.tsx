/**
 * Head: what the page's head says of it, drawn from anywhere in the tree (React puts it in the
 * document's head): the title the browser's tab and history show, and the description.
 */
export function Head({ title, description }: { readonly title: string; readonly description?: string }) {
  return (
    <>
      <title>{title}</title>
      {description !== undefined && <meta name="description" content={description} />}
    </>
  )
}
