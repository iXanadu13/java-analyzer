# Salsa Quick Reference Guide

## Adding a New Query

### 1. Simple Query (No Complex Types)

```rust
// In src/salsa_queries/parse.rs or similar

#[salsa::tracked]
pub fn my_query(db: &dyn Db, file: SourceFile) -> SimpleResult {
    let content = file.content(db);
    // ... compute result ...
    SimpleResult { /* ... */ }
}
```

**Requirements:**
- Return type must implement `PartialEq`, `Eq`, `Hash`, `Clone`
- Use primitives, `Arc<T>`, `Option<T>`, `Vec<T>` where T meets requirements

### 2. Query Returning Complex Data

```rust
// Use a lightweight wrapper for tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MyQueryResult {
    pub hash: u64,
    pub count: usize,
}

#[salsa::tracked]
pub fn my_query(db: &dyn Db, file: SourceFile) -> MyQueryResult {
    let data = compute_complex_data(db, file);
    MyQueryResult {
        hash: compute_hash(&data),
        count: data.len(),
    }
}

// Separate function to get actual data
pub fn get_my_data(db: &dyn Db, file: SourceFile) -> Vec<ComplexType> {
    compute_complex_data(db, file)
}
```

### 3. Query Accessing Workspace Index

```rust
// Don't use #[salsa::tracked] - workspace index is already managing state
pub fn query_workspace(db: &dyn Db, module_id: ModuleId) -> Arc<SomeData> {
    let workspace = db.workspace_index();
    let index = workspace.read();
    
    // Query the index
    index.get_something(module_id)
}
```

## Using Queries in LSP Handlers

### Pattern 1: File Changed (did_open, did_change)

```rust
async fn did_change(&self, params: DidChangeTextDocumentParams) {
    // 1. Update Salsa file
    let salsa_file = self.workspace.get_or_create_salsa_file(
        &uri, &new_content, &lang_id
    );
    
    // 2. Trigger change detection
    let classes = {
        let db = self.workspace.salsa_db.lock();
        
        // This is memoized - returns instantly if content unchanged
        let _result = salsa_queries::index::extract_classes(&*db, salsa_file);
        
        // Get actual data
        salsa_queries::index::get_extracted_classes(&*db, salsa_file)
    };
    
    // 3. Update workspace index
    self.workspace.index.write().await.update_source_in_context(
        module, source_root, origin, classes
    );
}
```

### Pattern 2: Query for Completion/Hover

```rust
async fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    
    // Get Salsa file
    let salsa_file = self.workspace.get_salsa_file(uri)?;
    
    let db = self.workspace.salsa_db.lock();
    
    // Query package (memoized)
    let package = salsa_queries::parse::extract_package(&*db, salsa_file);
    
    // Query imports (memoized)
    let imports = salsa_queries::parse::extract_imports(&*db, salsa_file);
    
    // Use results...
}
```

## Testing Queries

### Test Memoization

```rust
#[test]
fn test_my_query_memoization() {
    let db = Database::default();
    let file = create_test_file(&db, "content");
    
    // First call
    let result1 = my_query(&db, file);
    
    // Second call - should be instant (cache hit)
    let result2 = my_query(&db, file);
    
    assert_eq!(result1, result2);
}
```

### Test Invalidation

```rust
#[test]
fn test_my_query_invalidation() {
    use salsa::Setter;
    
    let mut db = Database::default();
    let file = create_test_file(&db, "content1");
    
    let result1 = my_query(&db, file);
    
    // Change content
    file.set_content(&mut db).to("content2".to_string());
    
    // Should recompute
    let result2 = my_query(&db, file);
    
    assert_ne!(result1, result2);
}
```

## Common Patterns

### Calling Other Queries

```rust
#[salsa::tracked]
pub fn high_level_query(db: &dyn Db, file: SourceFile) -> Result {
    // Salsa automatically tracks dependencies
    let package = extract_package(db, file);
    let imports = extract_imports(db, file);
    let classes = extract_classes(db, file);
    
    // Combine results
    Result { package, imports, classes }
}
```

### Conditional Computation

```rust
#[salsa::tracked]
pub fn conditional_query(db: &dyn Db, file: SourceFile) -> Option<Data> {
    let lang_id = file.language_id(db);
    
    if lang_id.as_ref() == "java" {
        Some(compute_java_data(db, file))
    } else if lang_id.as_ref() == "kotlin" {
        Some(compute_kotlin_data(db, file))
    } else {
        None
    }
}
```

### Aggregating Multiple Files

```rust
#[salsa::tracked]
pub fn module_query(db: &dyn Db, module: Module) -> ModuleData {
    let files = module.source_files(db);
    
    let mut aggregated = Vec::new();
    for file in files.iter() {
        // Each file query is memoized independently
        let data = file_query(db, *file);
        aggregated.push(data);
    }
    
    ModuleData { aggregated }
}
```

## Performance Tips

### DO
- ✅ Keep queries small and focused
- ✅ Use `Arc` for shared data
- ✅ Return lightweight results from tracked queries
- ✅ Let Salsa handle caching automatically
- ✅ Test both memoization and invalidation

### DON'T
- ❌ Perform I/O in queries (use inputs instead)
- ❌ Access mutable global state
- ❌ Return large data structures directly
- ❌ Manually cache query results
- ❌ Call queries in loops without batching

## Debugging

### Enable Salsa Logging

```rust
// In your test or main
std::env::set_var("SALSA_LOG", "1");
```

### Check Query Execution

```rust
#[salsa::tracked]
pub fn debug_query(db: &dyn Db, file: SourceFile) -> Result {
    tracing::debug!("debug_query called for {:?}", file.file_id(db));
    // ... computation ...
}
```

### Verify Memoization

```rust
let start = std::time::Instant::now();
let result1 = my_query(&db, file);
let first_duration = start.elapsed();

let start = std::time::Instant::now();
let result2 = my_query(&db, file);
let second_duration = start.elapsed();

// Second call should be much faster
assert!(second_duration < first_duration / 10);
```

## Migration Checklist

When migrating existing code to use Salsa:

- [ ] Identify the input data (file content, configuration, etc.)
- [ ] Create Salsa input types (`#[salsa::input]`)
- [ ] Identify derived computations
- [ ] Create Salsa queries (`#[salsa::tracked]`)
- [ ] Update callers to use queries instead of direct computation
- [ ] Add tests for memoization and invalidation
- [ ] Verify performance improvement
- [ ] Update documentation

## Common Errors

### Error: "doesn't implement PartialEq"

**Solution**: Use a wrapper type or return Arc<T>

```rust
// Instead of:
#[salsa::tracked]
pub fn bad_query(db: &dyn Db, file: SourceFile) -> ComplexType { ... }

// Do:
#[salsa::tracked]
pub fn good_query(db: &dyn Db, file: SourceFile) -> Arc<ComplexType> { ... }
```

### Error: "doesn't implement Update"

**Solution**: Don't use `#[salsa::tracked]` for functions accessing mutable state

```rust
// Instead of:
#[salsa::tracked]
pub fn bad_query(db: &dyn Db) -> Data {
    db.workspace_index().read().get_data()
}

// Do:
pub fn good_query(db: &dyn Db) -> Data {
    db.workspace_index().read().get_data()
}
```

## Resources

- [Salsa Book](https://salsa-rs.github.io/salsa/)
- [rust-analyzer Architecture](https://github.com/rust-lang/rust-analyzer/blob/master/docs/dev/architecture.md)
- [Our Implementation](./SALSA_IMPLEMENTATION.md)
