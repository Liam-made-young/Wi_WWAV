// Opening an address in the person's own browser (Open in Gmail, a Brightspace
// link). The page asks the window to; in the app the opener plugin answers,
// where the window's capability allows it. Only web addresses are opened:
// a link Claude wrote is data, never a command.

export function openExternal(url: string): boolean {
  if (!/^https?:\/\//i.test(url)) return false;
  window.open(url, '_blank', 'noopener,noreferrer');
  return true;
}
