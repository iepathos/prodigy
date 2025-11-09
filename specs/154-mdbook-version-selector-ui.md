---
number: 154
title: mdBook Version Selector UI
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-11
---

# Specification 154: mdBook Version Selector UI

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Projects using mdBook for documentation often need to serve multiple versions (e.g., v0.2.6, v0.2.5, latest) to accommodate users on different software versions. Unlike MkDocs which has plugins like `mike` for version management, mdBook lacks built-in versioning support.

Currently, Prodigy's automated documentation workflow generates docs for the latest codebase only. Users cannot view documentation for older versions they may be using. This is a common pattern seen in projects like Rust's documentation (stable, beta, nightly) served at different subdirectories.

## Objective

Create a reusable version selector UI component for mdBook that:
- Displays a dropdown allowing users to switch between documentation versions
- Integrates seamlessly with mdBook's default themes
- Fetches version metadata from a central `versions.json` file
- Preserves the current page path when switching versions
- Works across all repositories using the book workflow system

## Requirements

### Functional Requirements

- **Version Dropdown**: Display a dropdown in the mdBook navigation bar with available versions
- **Version Metadata**: Fetch version information from `/versions.json` at the documentation root
- **Current Version Detection**: Automatically detect which version the user is viewing based on URL path
- **Page Preservation**: When switching versions, attempt to navigate to the same page in the target version
- **Fallback Handling**: If current page doesn't exist in target version, redirect to index
- **Visual Indicators**: Highlight the current version and mark the latest version
- **Responsive Design**: Work on mobile and desktop browsers
- **Theme Compatibility**: Integrate with mdBook's default themes (Rust, Navy, Ayu, Coal, Light)

### Non-Functional Requirements

- **Performance**: Version selector loads without blocking page render
- **Accessibility**: Keyboard navigable and screen reader compatible
- **Browser Support**: Works on modern browsers (Chrome, Firefox, Safari, Edge)
- **No External Dependencies**: Pure JavaScript, no frameworks required
- **Minimal Footprint**: < 5KB combined JS + CSS
- **Reusability**: Can be dropped into any mdBook project with minimal configuration

## Acceptance Criteria

- [ ] JavaScript component (`version-selector.js`) fetches and parses `versions.json`
- [ ] CSS stylesheet (`version-selector.css`) provides consistent styling across themes
- [ ] Dropdown renders in mdBook navigation bar (top-right area)
- [ ] Current version is visually highlighted in the dropdown
- [ ] Latest version shows "(Latest)" label
- [ ] Clicking a version navigates to that version preserving current chapter path
- [ ] Component gracefully handles missing `versions.json` (no error displayed)
- [ ] Dropdown is keyboard accessible (Tab, Enter, Arrow keys)
- [ ] Works on mobile devices (touch-friendly, responsive)
- [ ] Integration instructions included in README or docs
- [ ] Example `versions.json` schema documented

## Technical Details

### Implementation Approach

**Version Selector Component Architecture**:
```
┌─────────────────────────────────────┐
│  mdBook Page                        │
│  ┌───────────────────────────────┐  │
│  │ Navigation Bar                │  │
│  │  ┌──────────────────────────┐ │  │
│  │  │ Version Selector (v0.2.6)│ │  │
│  │  │  ▼ v0.2.6 (Latest)       │ │  │
│  │  │    v0.2.5                │ │  │
│  │  │    v0.2.4                │ │  │
│  │  └──────────────────────────┘ │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
         │
         │ Fetches on load
         ▼
    /versions.json
    {
      "latest": "v0.2.6",
      "versions": [
        {"version": "v0.2.6", "path": "/v0.2.6/", ...},
        {"version": "v0.2.5", "path": "/v0.2.5/", ...}
      ]
    }
```

### File Structure

**Theme Files** (added to any mdBook project):
```
book/theme/
├── version-selector.js     # Version selector logic
├── version-selector.css    # Dropdown styling
└── versions.json           # Version metadata (at book root)
```

**mdBook Configuration** (book/book.toml):
```toml
[output.html]
additional-css = ["theme/version-selector.css"]
additional-js = ["theme/version-selector.js"]
```

### versions.json Schema

```json
{
  "latest": "v0.2.6",
  "versions": [
    {
      "version": "v0.2.6",
      "path": "/v0.2.6/",
      "label": "v0.2.6 (Latest)",
      "released": "2025-01-15"
    },
    {
      "version": "v0.2.5",
      "path": "/v0.2.5/",
      "label": "v0.2.5",
      "released": "2025-01-10"
    }
  ]
}
```

### JavaScript Implementation Strategy

**version-selector.js** key functions:
- `fetchVersions()`: Async fetch `/versions.json` with error handling
- `detectCurrentVersion()`: Parse window.location.pathname to extract version
- `createDropdown(versions, current)`: Build <select> element with version options
- `insertIntoNavbar(dropdown)`: Inject dropdown into mdBook's `.nav-wrapper`
- `handleVersionChange(event)`: Navigate to selected version, preserving page path
- `preservePagePath(targetVersionPath)`: Extract current chapter and append to target

**Event Handling**:
- DOMContentLoaded: Initialize selector after page loads
- Change event: Handle version selection
- Error handling: Gracefully degrade if versions.json missing

### CSS Styling Strategy

**version-selector.css** responsibilities:
- Style dropdown to match mdBook theme aesthetics
- Position in navigation bar (top-right, before search)
- Responsive design for mobile (optional: hamburger menu integration)
- Hover and focus states for accessibility
- Theme-aware colors (use CSS variables if available)

### Integration Pattern

**For any mdBook project**:
1. Copy `version-selector.js` and `version-selector.css` to `book/theme/`
2. Add `additional-js` and `additional-css` to `book.toml`
3. Create `versions.json` at documentation root
4. Deploy with GitHub Pages or other hosting

**Automated Setup** (future enhancement):
```bash
prodigy init-book --with-versioning
# Copies theme files and creates example versions.json
```

## Dependencies

**Prerequisites**: None (standalone component)

**Affected Components**:
- mdBook theme directory (new files added)
- mdBook configuration (book.toml updated)
- GitHub Pages deployment (versions.json served from root)

**External Dependencies**: None (pure JavaScript and CSS)

## Testing Strategy

### Unit Tests
- Mock `fetch()` to test versions.json parsing
- Test `detectCurrentVersion()` with various URL patterns
- Test `preservePagePath()` path extraction logic
- Test dropdown rendering with different version counts

### Integration Tests
- Test in actual mdBook environment with multiple versions
- Verify navigation between versions preserves page path
- Test fallback when target page doesn't exist in version
- Validate across different mdBook themes

### Browser Compatibility Tests
- Chrome, Firefox, Safari, Edge (latest versions)
- Mobile browsers (iOS Safari, Chrome Mobile)
- Accessibility testing with screen readers
- Keyboard navigation testing

### User Acceptance
- Manual testing: switch between versions, verify page preservation
- Visual testing: dropdown appearance in all themes
- Performance testing: measure load time impact

## Documentation Requirements

### Code Documentation
- JSDoc comments for all functions in version-selector.js
- Inline comments explaining version detection logic
- CSS comments explaining theme integration

### User Documentation
- **README.md** in `book/theme/`: Setup instructions for version selector
- **versions.json schema**: Documented structure and fields
- **Integration guide**: Step-by-step instructions for adding to mdBook projects
- **Customization guide**: How to style dropdown for custom themes

### Architecture Updates
- Update `automated-documentation.md` with versioning section
- Add example configurations for different project types
- Document how version selector integrates with book workflow

## Implementation Notes

### Browser Compatibility Considerations
- Use `fetch()` API (ES6, supported in all modern browsers)
- Gracefully degrade if `fetch()` unavailable (older browsers)
- Use CSS Grid/Flexbox for layout (widely supported)
- Avoid experimental CSS features

### Performance Optimization
- Fetch `versions.json` once on page load, cache in memory
- Use lightweight DOM manipulation (no jQuery)
- Lazy-load if version selector not immediately visible (future)

### Security Considerations
- Sanitize version strings before inserting into DOM
- Use relative paths to prevent open redirects
- Validate versions.json structure before rendering

### Accessibility Best Practices
- Use semantic HTML (`<select>` element)
- Include ARIA labels for screen readers
- Ensure keyboard navigation (Tab, Enter, Esc)
- Provide visible focus indicators

## Migration and Compatibility

### Backward Compatibility
- Existing mdBook projects work without changes (no breaking changes)
- Version selector is opt-in (requires manual integration)
- If `versions.json` missing, component silently skips rendering

### Migration Path for Existing Projects
1. Add `version-selector.js` and `.css` to `book/theme/`
2. Update `book.toml` with `additional-js` and `additional-css`
3. Create `versions.json` with current version as only entry
4. Deploy and verify dropdown appears
5. Add historical versions as they are built

### Future Enhancements
- Prodigy CLI command: `prodigy book add-versioning`
- Automatic `versions.json` generation from git tags
- Version comparison view (diff between versions)
- Search integration (search across all versions)

## File Locations

**Reusable Theme Components**:
- `book/theme/version-selector.js` - Version selector JavaScript
- `book/theme/version-selector.css` - Version selector styles
- `book/theme/README.md` - Integration and usage instructions

**Example Configuration**:
- `examples/versioned-book/` - Complete example with versions.json
- `examples/versioned-book/book.toml` - Configuration example

**Documentation**:
- `book/src/automated-documentation.md` - Updated with versioning guide
- `book/src/versioning-setup.md` - Detailed versioning setup guide (new)

## Success Metrics

- Version selector successfully integrated into Prodigy's own documentation
- At least 3 versions deployed and switchable
- Page path preserved when switching between versions >90% of time
- Zero JavaScript errors in browser console
- Component reused in at least one external project
- Documentation receives positive feedback on clarity
