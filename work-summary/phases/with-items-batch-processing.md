# With-Items Batch Processing Implementation

**Date**: 2024-01-XX  
**Component**: Workflow Executor  
**Status**: Completed ✅

## Overview

Implemented batch processing functionality for workflow `with-items` iteration to enable consistent parameter passing and efficient processing of large datasets.

## Problem Statement

Previously, the `with-items` workflow feature lacked batch processing capabilities. The requirement was to:

1. **Maintain backward compatibility**: Continue processing items individually by default
2. **Add batch processing**: Enable grouping items into batches when `batch_size` is specified
3. **Efficient bulk operations**: Allow actions to process multiple items at once when supported

## Solution

Modified the workflow task executor to support two modes based on the presence of `batch_size`:

### Key Changes

1. **Individual Processing (default)**: Without `batch_size`, items are processed one at a time (backward compatible)
2. **Batch Processing**: When `batch_size` is specified, items are grouped into arrays and processed as batches
3. **Flexible Batch Sizes**: The final batch can be smaller than `batch_size`
4. **Concurrency Control**: Both modes respect the `concurrency` setting for parallel execution

## Implementation Details

### Code Changes

**File**: `crates/executor/src/workflow/task_executor.rs`

- **Modified `execute_with_items()` method**:
  - Split into two execution paths based on `batch_size` presence
  - **Without `batch_size`**: Iterates over items individually (original behavior)
  - **With `batch_size`**: Creates batches and processes them as arrays
  - The `item` context variable receives either a single value or an array depending on mode
  - The `index` context variable receives either item index or batch index depending on mode

### Algorithm

```rust
if let Some(batch_size) = task.batch_size {
    // Batch mode: split items into batches and pass as arrays
    let batches: Vec<Vec<JsonValue>> = items
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();
    
    for (batch_idx, batch) in batches.into_iter().enumerate() {
        // Set current_item to the batch array
        context.set_current_item(json!(batch), batch_idx);
        // Execute action with batch
    }
} else {
    // Individual mode: process each item separately
    for (item_idx, item) in items.into_iter().enumerate() {
        // Set current_item to the individual item
        context.set_current_item(item, item_idx);
        // Execute action with single item
    }
}
```

## Usage Examples

### Without batch_size (individual processing)

```yaml
tasks:
  - name: deploy_to_regions
    action: cloud.deploy_instance
    with_items: "{{ parameters.regions }}"
    input:
      region: "{{ item }}"  # Single region value
```

### With batch_size (batch processing)

```yaml
tasks:
  - name: process_large_dataset
    action: data.transform
    with_items: "{{ vars.records }}"
    batch_size: 100  # Process 100 items at a time
    concurrency: 5   # Process 5 batches concurrently
    input:
      records: "{{ item }}"  # Array of up to 100 records (batch)
```

### Comparison

```yaml
# Individual: one API call per region
- with_items: "{{ parameters.regions }}"
  input:
    region: "{{ item }}"  # "us-east-1"

# Batch: one API call per 10 regions  
- with_items: "{{ parameters.regions }}"
  batch_size: 10
  input:
    regions: "{{ item }}"  # ["us-east-1", "us-west-2", ...]
```

## Testing

### Unit Tests Added

**File**: `crates/executor/src/workflow/task_executor.rs`

1. **`test_with_items_batch_creation`**: Verifies batches are created correctly with specified batch_size
2. **`test_with_items_no_batch_size_individual_processing`**: Verifies items processed individually when batch_size not specified
3. **`test_with_items_batch_vs_individual`**: Verifies different behavior between batch and individual modes

### Test Results

```
test workflow::task_executor::tests::test_with_items_batch_creation ... ok
test workflow::task_executor::tests::test_with_items_no_batch_size_individual_processing ... ok
test workflow::task_executor::tests::test_with_items_batch_vs_individual ... ok
```

All existing executor tests pass (55 unit tests, 35 integration tests).

## Documentation Updates

**File**: `docs/workflow-orchestration.md`

- Updated section 2.2 "Iteration (with-items)" with batch processing behavior
- Clarified that `item` is individual value without `batch_size`, array with `batch_size`
- Updated special variables section to explain different modes
- Added comparison examples showing individual vs batch processing

## Benefits

1. **Backward Compatible**: Existing workflows continue to work without changes
2. **Efficiency**: Batch processing reduces overhead for large datasets when enabled
3. **Flexibility**: Choose between individual or batch processing per task
4. **Performance**: Bulk API operations can process multiple items in one call

## Breaking Changes

✅ **No Breaking Changes**: This implementation is fully backward compatible.

### Migration Not Required

Existing workflows continue to work without modification:
- Without `batch_size`: items processed individually (existing behavior)
- With `batch_size`: opt-in to batch processing for new workflows

**To enable batch processing**:
```yaml
# Add batch_size to existing with-items task
with_items: "{{ parameters.regions }}"
batch_size: 10  # New parameter
input:
  regions: "{{ item }}"  # Now receives array instead of single value
```

## Performance Considerations

- **Trade-offs**: Individual processing gives fine-grained control; batch processing improves throughput
- **Concurrency**: Both modes support parallel execution via `concurrency` parameter
- **Memory**: Batch processing uses more memory per task but fewer total tasks
- **API efficiency**: Use batching when APIs support bulk operations to reduce network overhead

## Future Enhancements

Potential improvements for future consideration:

1. **Adaptive batching**: Automatically adjust batch size based on item size
2. **Partial batch retry**: Retry only failed items within a batch
3. **Streaming batches**: Support lazy evaluation for very large datasets
4. **Batch result aggregation**: Built-in functions to aggregate batch results

## References

- Task executor implementation: `crates/executor/src/workflow/task_executor.rs`
- Workflow context: `crates/executor/src/workflow/context.rs`
- Documentation: `docs/workflow-orchestration.md`
- Data models: `crates/common/src/workflow/parser.rs`

## Completion Checklist

- [x] Implementation completed (both individual and batch modes)
- [x] Unit tests added and passing
- [x] Documentation updated
- [x] All existing tests passing (backward compatible)
- [x] No breaking changes - fully backward compatible
- [x] Performance optimized with concurrency support
- [ ] Performance benchmarking (future work)