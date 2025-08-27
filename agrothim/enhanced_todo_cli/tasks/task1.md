
## 📋 **PHASE 1: Foundation (Week 1)**

### **Task 1.1: Project Setup & Docker Environment**
**Time:** 30-45 minutes
**Dependencies:** None

#### **Acceptance Criteria:**
- [x] Cargo project created with proper structure
- [x] Docker Compose file working with PostgreSQL
- [x] Database container starts successfully
- [x] Can connect to database from host machine
- [x] Environment variables loaded properly

#### **Implementation Steps:**
```bash
1. cargo new enhanced-todo-cli
2. Create docker-compose.yml
3. Create .env file with DATABASE_URL
4. docker-compose up -d postgres
5. Test connection: docker-compose exec postgres psql -U todo_user -d todo_cli
```

#### **Success Test:**
```bash
docker-compose ps  # Should show postgres running
psql -h localhost -U todo_user -d todo_cli -c "SELECT 1;"  # Should return 1
```

---

### **Task 1.2: Basic Dependencies & Project Structure**
**Time:** 20-30 minutes
**Dependencies:** Task 1.1

#### **Acceptance Criteria:**
- [ ] Cargo.toml with all necessary dependencies
- [ ] Module structure created (folders + mod.rs files)
- [ ] Project compiles without errors
- [ ] Basic logging setup working

#### **File Structure:**
```
src/
├── main.rs
├── lib.rs
├── models/mod.rs
├── database/mod.rs
├── services/mod.rs
├── cli/mod.rs
└── utils/mod.rs
```

#### **Success Test:**
```bash
cargo check  # Should compile without errors
cargo run    # Should run without panic (even if does nothing)
```

---

### **Task 1.3: Database Connection & Health Check**
**Time:** 45-60 minutes
**Dependencies:** Task 1.2

#### **Acceptance Criteria:**
- [x] SQLx connection pool established
- [x] Connection function with proper error handling
- [x] Health check query works
- [x] Connection failures handled gracefully
- [x] Connection details logged properly

#### **Key Components:**
```rust
// src/database/connection.rs
- Database struct with PgPool
- new() method with connection logic
- health_check() method
- Proper error handling with anyhow
```

#### **Success Test:**
```rust
// Should work in main.rs:
let db = Database::new(&database_url).await?;
let is_healthy = db.health_check().await?;
assert!(is_healthy);
```

---

### **Task 1.4: First Migration & Schema**
**Time:** 30-45 minutes
**Dependencies:** Task 1.3

#### **Acceptance Criteria:**
- [ ] Migration directory created
- [ ] 001_initial_schema.sql with users & tasks tables
- [ ] Migration runs successfully
- [ ] Tables created with proper constraints
- [ ] Indexes created for performance

#### **Schema Requirements:**
```sql
Users: id (UUID), username, email, password_hash, timestamps
Tasks: id (UUID), title, description, completed, priority, due_date, user_id, timestamps
Proper foreign keys, indexes, and constraints
```

#### **Success Test:**
```bash
sqlx migrate run  # Should execute without errors
\dt  # Should show users and tasks tables
SELECT COUNT(*) FROM users;  # Should work (return 0)
```

---

### **Task 1.5: User Model & Validation**
**Time:** 45-60 minutes
**Dependencies:** Task 1.4

#### **Acceptance Criteria:**
- [ ] User struct with all fields
- [ ] Proper serde serialization/deserialization
- [ ] Validation rules (email format, username length)
- [ ] Password hashing functionality
- [ ] CreateUserRequest & UpdateUserRequest DTOs
- [ ] Unit tests for validation logic

#### **Key Components:**
```rust
// src/models/user.rs
- User struct mapping to database
- CreateUserRequest with validation
- Password hashing with bcrypt
- Email validation
- Unit tests for edge cases
```

#### **Success Test:**
```rust
let user = User::new("testuser", "test@example.com", "password123");
assert!(user.verify_password("password123"));
assert!(!user.verify_password("wrong"));
```

---

### **Task 1.6: Task Model & Business Logic**
**Time:** 45-60 minutes
**Dependencies:** Task 1.5

#### **Acceptance Criteria:**
- [ ] Task struct with all fields
- [ ] TaskStatus and TaskPriority enums
- [ ] Business logic methods (complete, is_overdue, etc.)
- [ ] CreateTaskRequest & UpdateTaskRequest DTOs
- [ ] Validation rules for all fields
- [ ] Unit tests for business logic

#### **Business Rules:**
```rust
- Task title: 1-255 characters, required
- Description: optional, max 1000 characters
- Priority: Low/Medium/High, default Medium
- Status: Pending/InProgress/Completed
- Due date: optional, must be future date
- Auto-set completed_at when status = Completed
```

#### **Success Test:**
```rust
let mut task = Task::new("Test task", user_id);
assert_eq!(task.status, TaskStatus::Pending);
task.complete();
assert_eq!(task.status, TaskStatus::Completed);
assert!(task.completed_at.is_some());
```