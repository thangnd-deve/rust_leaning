## 📊 **PHASE 2: Data Layer (Week 1-2)**

### **Task 2.1: User Repository Implementation**
**Time:** 60-75 minutes
**Dependencies:** Task 1.6

#### **Acceptance Criteria:**
- [x] UserRepository trait defined
- [x] PostgresUserRepository implementation
- [x] CRUD operations: create, find_by_id, find_by_username, update
- [x] Proper SQL queries with parameter binding
- [x] Error handling for unique constraint violations
- [x] Integration tests with test database

#### **Repository Methods:**
```rust
async fn create(&self, user: CreateUserRequest) -> Result<User>;
async fn find_by_id(&self, id: &str) -> Result<Option<User>>;
async fn find_by_username(&self, username: &str) -> Result<Option<User>>;
async fn update(&self, id: &str, updates: UpdateUserRequest) -> Result<User>;
```

#### **Success Test:**
```rust
let user = repo.create(CreateUserRequest { ... }).await?;
let found = repo.find_by_id(&user.id).await?;
assert_eq!(found.unwrap().username, user.username);
```

---

### **Task 2.2: Task Repository Implementation**
**Time:** 90-120 minutes
**Dependencies:** Task 2.1

#### **Acceptance Criteria:**
- [x] TaskRepository trait defined
- [x] PostgresTaskRepository implementation
- [x] CRUD operations with proper filtering
- [x] Complex queries: find_by_user, find_overdue, search
- [x] Pagination support for large task lists
- [x] Proper JOIN queries for user data
- [x] Integration tests covering all scenarios

#### **Repository Methods:**
```rust
async fn create(&self, task: CreateTaskRequest) -> Result<Task>;
async fn find_by_id(&self, id: &str) -> Result<Option<Task>>;
async fn find_by_user(&self, user_id: &str, filter: TaskFilter) -> Result<Vec<Task>>;
async fn update(&self, id: &str, updates: UpdateTaskRequest) -> Result<Task>;
async fn delete(&self, id: &str) -> Result<()>;
async fn find_overdue(&self, user_id: &str) -> Result<Vec<Task>>;
```

#### **Success Test:**
```rust
let task = repo.create(CreateTaskRequest { ... }).await?;
let tasks = repo.find_by_user(&user_id, TaskFilter::default()).await?;
assert_eq!(tasks.len(), 1);
```

---

### **Task 2.3: Database Integration Tests**
**Time:** 45-60 minutes
**Dependencies:** Task 2.2

#### **Acceptance Criteria:**
- [ ] Test database setup automated
- [ ] Test fixtures for common scenarios
- [ ] Integration tests for repository layer
- [ ] Transaction rollback for test isolation
- [ ] Performance tests for large datasets
- [ ] All tests pass consistently

#### **Test Coverage:**
```rust
- User creation with duplicate username/email
- Task creation with invalid user_id
- Complex filtering scenarios
- Edge cases (empty results, large datasets)
- Concurrent access scenarios
- Foreign key constraint validation
```

#### **Success Test:**
```bash
cargo test  # All tests should pass
cargo test --test integration_tests  # Specific integration tests
```

---