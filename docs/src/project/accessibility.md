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

The accessibility review itself is planned as part of the grant-funded work in
WP4, alongside contributor onboarding and security follow-up. That review will
turn these commitments into a concrete audit and remediation list for the web
UI and the documentation site.

For now, this statement is intentionally modest: the project is committing to
accessible implementation practices and an explicit review path, not claiming
WCAG conformance before the audit exists.
