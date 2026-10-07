// A control this build can't offer yet, shown disabled with the one
// sentence that says why (docs/SPEC.md 8.10), so nothing is hidden and
// nothing pretends.

export function NotYet({ label, why }: { label: string; why: string }) {
  return (
    <div className="setting">
      <button type="button" className="gel" disabled>
        {label}
      </button>
      <p className="why" data-text="secondary">
        {why}
      </p>
    </div>
  );
}
