# Web UI Pack Testing Integration

## Overview

The Attune web UI now includes comprehensive pack testing capabilities, allowing users to view test results, execute tests, and monitor pack quality through a visual interface.

## Features

### 1. Pack Detail Page - Test Results

The pack detail page displays the latest test results and provides quick access to test history.

**Location**: `/packs/{ref}`

**Features**:
- Latest test status badge (passed/failed/skipped)
- Test summary (total, passed, failed, skipped)
- Pass rate percentage
- Test execution timestamp
- "Run Tests" button for manual test execution
- Toggle between latest results and test history

### 2. Test Result Display Component

**Component**: `PackTestResult`

Displays detailed test execution results with:
- Overall status indicator (passed/failed)
- Test statistics (total, passed, failed, skipped)
- Pass rate and duration
- Expandable test suites
- Individual test case results with error messages
- stdout/stderr output for failed tests

**Usage**:
```tsx
import PackTestResult from '@/components/packs/PackTestResult';

<PackTestResult 
  result={testResultData} 
  showDetails={true} 
/>
```

### 3. Test History Component

**Component**: `PackTestHistory`

Displays a paginated list of all test executions for a pack.

**Features**:
- Chronological list of test executions
- Status badges for each execution
- Trigger reason indicators (register, manual, ci, schedule)
- Expandable details for each execution
- Load more pagination
- Date/time formatting
- Duration display

**Usage**:
```tsx
import PackTestHistory from '@/components/packs/PackTestHistory';

<PackTestHistory
  executions={testExecutions}
  isLoading={false}
  onLoadMore={() => loadNextPage()}
  hasMore={hasMorePages}
/>
```

### 4. Test Status Badge

**Component**: `PackTestBadge`

Compact status indicator showing test results.

**Variants**:
- `passed` - Green with checkmark
- `failed` - Red with X
- `skipped` - Gray with clock
- Size options: `sm`, `md`, `lg`

**Usage**:
```tsx
import PackTestBadge from '@/components/packs/PackTestBadge';

<PackTestBadge
  status="passed"
  passed={76}
  total={76}
  size="md"
  showCounts={true}
/>
```

### 5. Remote Pack Installation Page

**Location**: `/packs/install`

Browser-based pack installation is limited to sources the server can fetch from
remote locations: git repositories, archives, or configured pack registries.
Filesystem path registration is intentionally not exposed in the web client
because browser users cannot browse or validate server-local filesystem paths.

## API Integration

### React Query Hooks

#### `usePackLatestTest(packRef: string)`

Fetches the latest test result for a pack.

```tsx
const { data: latestTest } = usePackLatestTest('core');

// Returns: { data: PackTestExecution | null }
```

#### `usePackTestHistory(packRef: string, params)`

Fetches paginated test history for a pack.

```tsx
const { data: testHistory } = usePackTestHistory('core', {
  page: 1,
  pageSize: 10
});

// Returns: { data: { items: PackTestExecution[], meta: PaginationMeta } }
```

#### `useExecutePackTests()`

Mutation hook for executing pack tests.

```tsx
const executeTests = useExecutePackTests();

await executeTests.mutateAsync('core');
// Executes tests and invalidates test queries
```

## User Workflows

### View Test Results

1. Navigate to `/packs`
2. Click on a pack name
3. View latest test results in the main section
4. Click "View History" to see all test executions
5. Click on any execution to expand details

### Run Tests Manually

1. Navigate to pack detail page
2. Click "Run Tests" button in sidebar (or main section)
3. Wait for test execution (button shows "Running Tests...")
4. Results update automatically on completion
5. View detailed results in the expanded test result component

### Register a New Pack

1. Navigate to `/packs`
2. Click "+ Register Pack" button
3. Choose Git Repository, Archive URL, or Pack Registry
4. Enter or select the remote pack source
5. Optionally adjust install/test options
6. Click "Install Pack"
7. Wait for installation to complete
8. View results and test outcomes
9. Automatically redirected to pack details page

### Install Pack with Tests Disabled

Use this workflow during development:

1. Navigate to `/packs/install`
2. Enter or select the remote pack source
3. **Check "Skip Tests"**
4. Click "Install Pack"
5. Pack is installed without validation
6. Later, manually run tests from pack detail page

### Server-side Filesystem Registration

Use the CLI or API directly when a pack directory is already available on the
API server filesystem. The web client does not expose this flow.

## Visual Design

### Color Scheme

- **Passed Tests**: Green (`green-600`, `green-50`)
- **Failed Tests**: Red (`red-600`, `red-50`)
- **Skipped Tests**: Gray (`gray-600`, `gray-50`)
- **Trigger Types**:
  - Register: Blue
  - Manual: Purple
  - CI: Green
  - Schedule: Yellow

### Icons

- **CheckCircle** - Passed tests
- **XCircle** - Failed tests
- **Clock** - Skipped tests
- **AlertCircle** - Unknown/error state
- **Play** - Run tests button
- **Calendar** - Test execution date
- **Clock** - Test duration
- **ChevronDown/ChevronRight** - Expandable sections

## Error Handling

### Test Execution Failures

When tests fail:
- Error message displayed at top of page
- Red alert banner with error details
- Test results still stored (if execution completed)
- User can retry by clicking "Run Tests" again

### Registration Failures

When registration fails:
- Red alert box with error message
- Form remains filled for corrections
- Common errors:
  - "Pack directory does not exist"
  - "pack.yaml not found"
  - "Pack already exists" (suggest using force)
  - "Tests failed" (suggest using force or fixing tests)

### Network Errors

When API calls fail:
- Error toast/alert
- Retry button available
- State preserved for manual retry

## Accessibility

- All interactive elements keyboard accessible
- ARIA labels on icon buttons
- Color is not the only indicator (icons + text)
- Focus states clearly visible
- Proper heading hierarchy
- Screen reader friendly status messages

## Performance Considerations

### Data Fetching

- `staleTime: 30000` (30 seconds) on test queries
- Automatic refetch on window focus
- Pagination for test history (10 items per page)
- Query invalidation after test execution

### Optimizations

- Expandable sections to reduce initial render
- Lazy loading of test details
- Memoized components for large lists
- Efficient re-renders with React Query

## Responsive Design

- Mobile-friendly layouts
- Stacked columns on small screens
- Touch-friendly tap targets (min 44x44px)
- Horizontal scroll for wide tables
- Collapsible sections for mobile

## Future Enhancements

### Planned Features

1. **Real-time Test Execution**
   - WebSocket updates during test runs
   - Progress bar showing test completion
   - Live stdout/stderr streaming

2. **Test Comparison**
   - Compare test results across versions
   - Show performance regressions
   - Highlight newly failing tests

3. **Test Filtering**
   - Filter by status (passed/failed/skipped)
   - Filter by trigger type
   - Date range filtering
   - Search by test name

4. **Test Analytics**
   - Trend charts (pass rate over time)
   - Flaky test detection
   - Duration trends
   - Test coverage metrics

5. **Bulk Actions**
   - Run tests for multiple packs
   - Batch pack registration
   - Export test results (CSV, JSON)

6. **Notifications**
   - Browser notifications for test completion
   - Email alerts for test failures
   - Slack/webhook integrations

## Development Notes

### File Structure

```
web/src/
├── components/
│   └── packs/
│       ├── PackTestResult.tsx      # Detailed test result display
│       ├── PackTestBadge.tsx       # Status badge component
│       └── PackTestHistory.tsx     # Test history list
├── hooks/
│   └── usePackTests.ts             # React Query hooks
└── pages/
    └── packs/
        └── PackDetailPage.tsx      # Shows latest test results
```

### Adding New Components

1. Create component in `components/packs/`
2. Export from component file
3. Add to relevant pages
4. Update types if needed
5. Test build: `npm run build`

### API Client Updates

When backend API changes:

1. Start API server: `cargo run --bin attune-api`
2. Regenerate client: `cd web && npm run generate:api`
3. Update hook imports if service names changed
4. Update TypeScript types in hooks
5. Test and update components

### Testing

Manual testing checklist:

- [ ] Pack list page loads
- [ ] Pack detail page shows test results
- [ ] Test history displays correctly
- [ ] Run tests button works
- [ ] Pack registration form submits
- [ ] Success/error messages display
- [ ] Redirects work correctly
- [ ] Test details expand/collapse
- [ ] Status badges show correct colors
- [ ] Mobile layout works

## Troubleshooting

### "Cannot read property 'result' of undefined"

Test data not loaded yet. Add loading check:
```tsx
{latestTest?.data && <PackTestResult result={latestTest.data.result} />}
```

### Test history not updating

Manually invalidate queries after test execution:
```tsx
queryClient.invalidateQueries({ queryKey: ['pack-tests', packRef] });
```

### Styling not applied

- Check Tailwind classes are valid
- Verify `lucide-react` is installed
- Run `npm install` to ensure dependencies
- Clear build cache: `rm -rf dist && npm run build`

### API calls failing (401/403)

- Check access token in localStorage
- Verify token not expired
- Log in again to refresh token
- Check CORS configuration

## Related Documentation

- [Pack Testing Framework](./pack-testing-framework.md) - Overall testing design
- [Pack Testing User Guide](./PACK_TESTING.md) - CLI and API usage
- [Pack Testing API](./api-pack-testing.md) - API endpoint reference
- [Pack Install Integration](./pack-install-testing.md) - Installation with testing
- [Web UI Architecture](./web-ui-architecture.md) - Frontend architecture

## Changelog

- **2026-01-22**: Initial web UI integration
  - Pack test result display component
  - Test history component
  - Status badge component
  - Pack registration page
  - React Query hooks for test data
  - Integration with pack detail page
