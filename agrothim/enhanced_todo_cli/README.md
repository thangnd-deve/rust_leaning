# 🦀 Enhanced Todo CLI - Architecture & Design
*Week 3-4: Foundation Project - Architecture Analysis*

## 🎯 **Why This Project?**

### **Learning Objectives:**
- **Database Integration**: Từ memory sang persistent storage
- **Authentication Patterns**: JWT và session management basics  
- **API Design**: Chuẩn bị cho web development
- **Error Handling**: Production-ready error management
- **Testing Strategy**: Unit + Integration tests
- **Code Organization**: Module structure cho large projects

### **Career Relevance:**
- **Real-world Skills**: Mọi backend app đều có CRUD + Auth + Database
- **Architecture Thinking**: Tách concerns, loose coupling
- **Production Mindset**: Logging, config, error handling
- **Portfolio Value**: Demonstrates full-stack thinking

---

## 🏗️ **Architecture Analysis**

### **1. Layered Architecture Pattern**
```
┌─────────────────────────────────────┐
│           CLI Interface             │  ← User interaction
├─────────────────────────────────────┤
│          Service Layer              │  ← Business logic
├─────────────────────────────────────┤
│         Repository Layer            │  ← Data access
├─────────────────────────────────────┤
│          Database Layer             │  ← Persistence
└─────────────────────────────────────┘
```

**Why This Architecture?**
- **Separation of Concerns**: Mỗi layer có responsibility riêng
- **Testability**: Mock từng layer độc lập
- **Maintainability**: Change database không affect business logic
- **Scalability**: Dễ add HTTP API layer sau này

### **2. Module Structure Logic**
```
src/
├── cli/           ← User Interface Layer
├── services/      ← Business Logic Layer  
├── database/      ← Data Access Layer
├── models/        ← Domain Models (shared)
├── auth/          ← Cross-cutting Concern
├── utils/         ← Shared Utilities
└── api/           ← Future HTTP API Layer
```

**Why This Structure?**
- **Domain-Driven Design**: Models reflect business domain
- **Clean Dependencies**: Services depend on models, not CLI
- **Future-Proof**: Easy to add web API without restructuring
- **Rust Best Practices**: Clear module boundaries

---

## 📦 **Package Selection & Rationale**

### **Database: SQLx vs Diesel**
**Chosen: SQLx**
```toml
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono", "uuid"] }
```

**Why SQLx?**
- ✅ **Async-First**: Future-ready cho web development
- ✅ **Compile-Time Verification**: SQL queries checked at compile time
- ✅ **Raw SQL Control**: No ORM magic, learn actual SQL
- ✅ **Migration System**: Database versioning built-in
- ❌ **Learning Curve**: Need to know SQL (good for learning)

**Alternative: Diesel**
- ✅ **Type Safety**: Strong typing, no SQL injection
- ✅ **Mature**: Battle-tested trong production
- ❌ **Sync-Only**: Không fit với async ecosystem
- ❌ **Complex**: DSL learning overhead

### **CLI: Clap vs Structopt**
**Chosen: Clap v4**
```toml
clap = { version = "4.4", features = ["derive", "color"] }
```

**Why Clap?**
- ✅ **Derive Macros**: Clean, declarative API definition
- ✅ **Rich Features**: Subcommands, validation, help generation
- ✅ **Active Development**: Clap v4 là current standard
- ✅ **Ecosystem**: Integrates well với other tools

### **Authentication: JWT vs Session**
**Chosen: JWT (jsonwebtoken)**
```toml
jsonwebtoken = "9.2"
```

**Why JWT cho CLI?**
- ✅ **Stateless**: Không cần session storage
- ✅ **Future-Ready**: Web APIs thường dùng JWT
- ✅ **Learning Value**: Industry standard authentication
- ✅ **Portable**: Token có thể dùng across services

### **Async Runtime: Tokio vs async-std**
**Chosen: Tokio**
```toml
tokio = { version = "1.0", features = ["full"] }
```

**Why Tokio?**
- ✅ **Ecosystem Leader**: Most crates support Tokio
- ✅ **Production Ready**: Used by Discord, Dropbox, etc.
- ✅ **Rich Features**: Timers, filesystem, networking
- ✅ **Future Compatibility**: Cho web development sau này

### **Error Handling: anyhow vs thiserror**
**Chosen: Both**
```toml
anyhow = "1.0"      # For application errors
thiserror = "1.0"   # For library errors
```

**Why Both?**
- **anyhow**: Application-level error propagation (main, CLI)
- **thiserror**: Library-level custom errors (services, database)
- **Best Practice**: Different tools for different contexts
- **Learning Value**: Understand error handling patterns

---

## 🎨 **Design Patterns Analysis**

### **1. Repository Pattern**
```rust
// Why this pattern?
trait TaskRepository {
    async fn create(&self, task: &Task) -> Result<Task>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Task>>;
    async fn find_by_user(&self, user_id: &str, filter: &TaskFilter) -> Result<Vec<Task>>;
    // ...
}
```

**Benefits:**
- **Testable**: Mock repository cho unit tests
- **Swappable**: Change từ SQLite sang PostgreSQL dễ dàng
- **Clean**: Business logic không biết về SQL details
- **Standard Pattern**: Industry best practice

### **2. Service Layer Pattern**
```rust
// Why separate service layer?
struct TaskService {
    repository: Box<dyn TaskRepository>,
    auth: AuthService,
}

impl TaskService {
    async fn create_task(&self, user_id: &str, request: CreateTaskRequest) -> Result<Task> {
        // 1. Validate business rules
        // 2. Check permissions  
        // 3. Transform data
        // 4. Call repository
        // 5. Log/metrics
    }
}
```

**Benefits:**
- **Business Logic Centralization**: Rules ở một chỗ
- **Transaction Management**: Handle complex operations
- **Authorization**: Check permissions before data access
- **Reusability**: CLI và API cùng dùng service

### **3. Command Pattern (CLI)**
```rust
// Why command pattern?
enum Command {
    Add(AddCommand),
    List(ListCommand), 
    Complete(CompleteCommand),
    // ...
}
```

**Benefits:**
- **Extensibility**: Easy to add new commands
- **Validation**: Each command validates own inputs
- **Clean Mapping**: Command → Service calls
- **Help Generation**: Auto-generated help text

---

## 🗄️ **Database Design Decisions**

### **Schema Design Philosophy**
```sql
-- Why these design choices?

-- UUIDs instead of auto-increment IDs
id TEXT PRIMARY KEY  -- UUID as string

-- Explicit foreign key constraints
FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE

-- Check constraints for enums
CHECK (status IN ('pending', 'in_progress', 'completed'))

-- Proper indexing strategy
CREATE INDEX idx_tasks_user_id ON tasks(user_id);
```

**Rationale:**
- **UUIDs**: Distributed systems friendly, no collision
- **Foreign Keys**: Data integrity, prevents orphaned records
- **Check Constraints**: Database-level validation
- **Indexes**: Query performance optimization

### **Migration Strategy**
**Why SQLx Migrations?**
- **Version Control**: Database schema in git
- **Reproducible**: Same schema across environments
- **Rollback Support**: Can undo changes safely
- **Team Collaboration**: No schema drift issues

---

## 🔐 **Authentication Architecture**

### **JWT Strategy for CLI**
```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Login     │───▶│  Validate   │───▶│  Generate   │
│ Credentials │    │Credentials  │    │ JWT Token   │
└─────────────┘    └─────────────┘    └─────────────┘
                                            │
                                            ▼
                                    ┌─────────────┐
                                    │Store Token  │
                                    │   Locally   │
                                    └─────────────┘
```

**Design Decisions:**
- **Local Storage**: `~/.config/todo-cli/session.json`
- **Token Expiration**: 7 days (reasonable for CLI)
- **Refresh Strategy**: Simple re-login (không complex như web)
- **Security**: File permissions 600 (user-only read/write)

### **Password Security**
- **Hashing**: bcrypt with cost factor 12
- **Salt**: Automatic với bcrypt
- **No Plain Text**: Never store passwords directly

---

## 🧪 **Testing Strategy**

### **Testing Pyramid**
```
         ┌─────────────────┐
         │  Integration    │  ← 20% (CLI commands end-to-end)
         │     Tests       │
         ├─────────────────┤
         │  Service Tests  │  ← 50% (Business logic)
         │                 │
         ├─────────────────┤
         │   Unit Tests    │  ← 30% (Models, utilities)
         │                 │
         └─────────────────┘
```

**Test Types:**
- **Unit Tests**: Models validation, utility functions
- **Service Tests**: Business logic với mocked repositories
- **Integration Tests**: Full CLI commands với test database
- **Property Tests**: Edge cases với quickcheck (optional)

### **Test Database Strategy**
- **In-Memory SQLite**: Fast, isolated tests
- **Test Fixtures**: Seed data cho consistent tests
- **Cleanup**: Mỗi test có fresh database state
- **Parallel Execution**: Tests không affect each other

---

## ⚙️ **Configuration Management**

### **Configuration Layers**
```
Priority (High to Low):
1. Command-line arguments    (--database-url)
2. Environment variables     (TODO_DATABASE_URL)  
3. User config file         (~/.config/todo-cli/config.toml)
4. Default values           (built-in defaults)
```

**Why This Approach?**
- **Flexibility**: Different configs for different environments
- **Security**: Sensitive data từ env vars, không commit
- **User Experience**: Sane defaults, minimal configuration
- **Production Ready**: Industry standard approach

### **Config Structure**
```toml
# Example config.toml
[database]
url = "sqlite:~/.local/share/todo-cli/tasks.db"
pool_size = 5

[auth]
token_expiry_days = 7
secret_key = "your-secret-key"  # From env var in production

[logging]
level = "info"
format = "json"  # or "pretty"

[api]
enabled = false
port = 8080
host = "127.0.0.1"
```

---

## 📊 **Performance Considerations**

### **Database Optimization**
- **Connection Pooling**: SQLx pool size = 5 (reasonable for CLI)
- **Prepared Statements**: Automatic với SQLx
- **Indexes**: On user_id, status, due_date (common query patterns)
- **Batch Operations**: For bulk imports/exports

### **Memory Management**
- **Streaming Results**: Don't load all tasks into memory
- **Lazy Loading**: Tags loaded only when needed
- **Connection Limits**: Pool prevents connection exhaustion

---

## 🚀 **Future Extensions (Week 5+ prep)**

### **API Mode Foundation**
Project structure already supports:
```rust
// Future HTTP API endpoints
POST   /api/tasks              // Create task
GET    /api/tasks              // List tasks (with filters)
GET    /api/tasks/{id}         // Get specific task
PUT    /api/tasks/{id}         // Update task
DELETE /api/tasks/{id}         // Delete task
GET    /api/users/me/tasks     // User's tasks
```

### **Extensibility Points**
- **Plugin System**: Command registration mechanism
- **Export Formats**: Easy to add new formats (XML, YAML)
- **Storage Backends**: Abstract repository cho PostgreSQL, MongoDB
- **Notification System**: Email, Slack integrations

---

## 🎯 **Learning Outcomes**

Sau khi complete project này, bạn sẽ master:

### **Rust Skills:**
- ✅ **Async Programming**: tokio, async/await patterns
- ✅ **Database Integration**: SQLx, migrations, queries
- ✅ **Error Handling**: Custom errors, propagation strategies
- ✅ **Testing**: Unit, integration, mocking patterns
- ✅ **CLI Development**: Arguments parsing, user interaction

### **Backend Concepts:**
- ✅ **Architecture Patterns**: Layered architecture, separation of concerns
- ✅ **Authentication**: JWT tokens, password hashing
- ✅ **Data Modeling**: Relational design, foreign keys
- ✅ **Configuration**: Multi-layer config management
- ✅ **Logging**: Structured logging với tracing

### **Production Skills:**
- ✅ **Code Organization**: Module structure, dependency management
- ✅ **Database Migrations**: Schema versioning, rollback strategies
- ✅ **Security**: Authentication, input validation
- ✅ **Performance**: Indexing, connection pooling
- ✅ **Maintainability**: Clean code, documentation

---

**🎯 Next Steps:** Sau khi understand architecture này, bạn có thể start implement từng module một cách systematic. Recommend bắt đầu với models và database layer trước, sau đó services, cuối cùng CLI interface.

**💡 Key Insight:** Project này không chỉ là "todo app" - nó là foundation cho mọi backend application. Patterns và architecture ở đây sẽ apply cho social media API, e-commerce backend, và microservices sau này.