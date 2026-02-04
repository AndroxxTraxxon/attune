use attune_executor::workflow::context::WorkflowContext;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use std::collections::HashMap;

fn bench_context_clone_empty(c: &mut Criterion) {
    let ctx = WorkflowContext::new(json!({}), HashMap::new());

    c.bench_function("clone_empty_context", |b| b.iter(|| black_box(ctx.clone())));
}

fn bench_context_clone_with_results(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_with_task_results");

    for task_count in [10, 50, 100, 500].iter() {
        let mut ctx = WorkflowContext::new(json!({}), HashMap::new());

        // Simulate N completed tasks with 10KB results each
        for i in 0..*task_count {
            let large_result = json!({
                "status": "success",
                "output": vec![0u8; 10240], // 10KB
                "timestamp": "2025-01-17T00:00:00Z",
                "duration_ms": 1000,
            });
            ctx.set_task_result(&format!("task_{}", i), large_result);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(task_count),
            task_count,
            |b, _| b.iter(|| black_box(ctx.clone())),
        );
    }

    group.finish();
}

fn bench_with_items_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("with_items_simulation");

    // Simulate realistic workflow: 100 completed tasks, processing various list sizes
    let mut ctx = WorkflowContext::new(json!({}), HashMap::new());
    for i in 0..100 {
        ctx.set_task_result(&format!("task_{}", i), json!({"data": vec![0u8; 10240]}));
    }

    for item_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(item_count),
            item_count,
            |b, count| {
                b.iter(|| {
                    // Simulate what happens in execute_with_items
                    let mut clones = Vec::new();
                    for i in 0..*count {
                        let mut item_ctx = ctx.clone();
                        item_ctx.set_current_item(json!({"index": i}), i);
                        clones.push(item_ctx);
                    }
                    black_box(clones)
                })
            },
        );
    }

    group.finish();
}

fn bench_context_with_variables(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_with_variables");

    for var_count in [10, 50, 100].iter() {
        let mut vars = HashMap::new();
        for i in 0..*var_count {
            vars.insert(format!("var_{}", i), json!({"value": vec![0u8; 1024]}));
        }

        let ctx = WorkflowContext::new(json!({}), vars);

        group.bench_with_input(BenchmarkId::from_parameter(var_count), var_count, |b, _| {
            b.iter(|| black_box(ctx.clone()))
        });
    }

    group.finish();
}

fn bench_template_rendering(c: &mut Criterion) {
    let mut ctx = WorkflowContext::new(json!({"name": "test", "count": 42}), HashMap::new());

    // Add some task results
    for i in 0..10 {
        ctx.set_task_result(&format!("task_{}", i), json!({"result": i * 10}));
    }

    c.bench_function("render_simple_template", |b| {
        b.iter(|| black_box(ctx.render_template("Hello {{ parameters.name }}")))
    });

    c.bench_function("render_complex_template", |b| {
        b.iter(|| {
            black_box(ctx.render_template(
                "Name: {{ parameters.name }}, Count: {{ parameters.count }}, Result: {{ task.task_5.result }}"
            ))
        })
    });
}

criterion_group!(
    benches,
    bench_context_clone_empty,
    bench_context_clone_with_results,
    bench_with_items_simulation,
    bench_context_with_variables,
    bench_template_rendering,
);
criterion_main!(benches);
