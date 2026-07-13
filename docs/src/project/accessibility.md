# Accessibility

QueryFabric's current web surfaces are the demo UI in
[`crates/queryfabric-web`](../../../crates/queryfabric-web), the Leptos-based
editor components used by the demo UI, plus the documentation and website.

These surfaces have not yet been audited against WCAG. That is the honest
current state.

The project commits to a baseline that is practical for the codebase we have
today:

- semantic HTML instead of div-only layouts where the content model matters
- keyboard navigation for interactive controls and form flows
- sufficient contrast for text, controls, and status indicators
- clear labels, headings, and error messages for form-based interactions

The repository now has a deterministic structural gate for the generated
documentation in `checks.accessibility`. It verifies language metadata, page
titles, the main landmark, and `alt` attributes on generated images. This is a
smoke gate, not a WCAG conformance audit; keyboard, contrast, screen-reader,
and interactive-state review still require a human audit of the web UI.

For now, this statement is intentionally modest: the project is committing to
accessible implementation practices and an explicit review path, not claiming
WCAG conformance before the manual audit exists.
