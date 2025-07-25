# Feature: Claude Integration

## Objective
Create a sophisticated Claude CLI integration layer that maximizes the effectiveness of Claude interactions through intelligent prompting, context management, and response processing.

## Acceptance Criteria
- [ ] Advanced prompt engineering with templates
- [ ] Context window optimization
- [ ] Response parsing and validation
- [ ] Retry logic with exponential backoff
- [ ] Multiple Claude model support
- [ ] Token usage tracking and optimization
- [ ] Conversation memory management
- [ ] Custom Claude commands framework
- [ ] Response caching for efficiency

## Technical Details

### Prompt Engineering System

```rust
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<Variable>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub struct Variable {
    pub name: String,
    pub description: String,
    pub var_type: VariableType,
    pub required: bool,
    pub default: Option<String>,
}

pub enum VariableType {
    String,
    Number,
    Boolean,
    Code { language: String },
    File { path: PathBuf },
    Context { source: ContextSource },
}
```

Example prompt template:
```yaml
# templates/implement-feature.yaml
name: implement-feature
description: Implement a new feature based on specification
system_prompt: |
  You are an expert software engineer implementing features based on specifications.
  Follow best practices, write clean code, and ensure all acceptance criteria are met.

template: |
  ## Current Project Context
  Project: {project_name}
  Language: {primary_language}
  Framework: {framework}
  
  ## Feature Specification
  {spec_content}
  
  ## Current Implementation Status
  Completed specs: {completed_specs}
  Current iteration: {iteration}
  
  ## Previous Feedback
  {previous_feedback}
  
  ## Task
  Please implement or continue implementing this feature. Focus on:
  1. Meeting all acceptance criteria
  2. Following project conventions
  3. Writing maintainable code
  4. Adding appropriate tests
  
  {additional_instructions}

variables:
  - name: project_name
    type: context
    source: project.name
  - name: primary_language
    type: context
    source: project.language
  - name: additional_instructions
    type: string
    required: false
```

### Context Management

Intelligent context window optimization:

```rust
pub struct ContextManager {
    max_tokens: usize,
    priority_queue: BinaryHeap<ContextItem>,
}

pub struct ContextItem {
    pub content: String,
    pub priority: Priority,
    pub tokens: usize,
    pub source: ContextSource,
}

pub enum Priority {
    Critical = 1000,   // Must include (spec content, direct dependencies)
    High = 100,        // Should include (recent changes, related code)
    Medium = 10,       // Nice to have (project structure, conventions)
    Low = 1,           // Optional (historical context, examples)
}

impl ContextManager {
    pub fn build_context(&self, base_prompt: &str) -> Result<String> {
        let mut total_tokens = count_tokens(base_prompt);
        let mut context_items = Vec::new();
        
        while let Some(item) = self.priority_queue.pop() {
            if total_tokens + item.tokens > self.max_tokens {
                // Try to fit partial content for high priority items
                if item.priority >= Priority::High {
                    let truncated = self.truncate_intelligently(&item);
                    context_items.push(truncated);
                }
                break;
            }
            total_tokens += item.tokens;
            context_items.push(item);
        }
        
        Ok(self.format_context(context_items))
    }
}
```

### Response Processing

Advanced response parsing and validation:

```rust
pub struct ResponseProcessor {
    pub parsers: Vec<Box<dyn ResponseParser>>,
    pub validators: Vec<Box<dyn ResponseValidator>>,
}

pub trait ResponseParser {
    fn can_parse(&self, response: &str) -> bool;
    fn parse(&self, response: &str) -> Result<ParsedResponse>;
}

pub struct CodeBlockParser;
impl ResponseParser for CodeBlockParser {
    fn parse(&self, response: &str) -> Result<ParsedResponse> {
        // Extract code blocks with language detection
        // Handle multiple code blocks
        // Preserve formatting and indentation
    }
}

pub struct CommandParser;
impl ResponseParser for CommandParser {
    fn parse(&self, response: &str) -> Result<ParsedResponse> {
        // Parse mmm-specific commands from response
        // Examples: @mmm:complete, @mmm:needs-review, @mmm:blocked
    }
}
```

### Custom Claude Commands

Extensible command system:

```toml
# .mmm/commands.toml
[[commands]]
name = "implement"
aliases = ["impl", "i"]
prompt_template = "implement-feature"
pre_processors = ["gather-context", "analyze-dependencies"]
post_processors = ["extract-code", "update-state"]

[[commands]]
name = "review"
aliases = ["r"]
prompt_template = "code-review"
settings = { temperature = 0.3, max_tokens = 2000 }

[[commands]]
name = "debug"
aliases = ["d"]
prompt_template = "debug-issue"
interactive = true
```

### Token Optimization

Token usage tracking and optimization:

```rust
pub struct TokenTracker {
    pub daily_limit: Option<usize>,
    pub project_limits: HashMap<String, usize>,
    pub usage_db: TokenUsageDB,
}

impl TokenTracker {
    pub fn can_proceed(&self, estimated_tokens: usize) -> Result<bool> {
        // Check daily limits
        // Check project limits
        // Provide warnings at 80% usage
    }
    
    pub fn optimize_prompt(&self, prompt: &str) -> String {
        // Remove redundant whitespace
        // Compress repetitive content
        // Use references for large code blocks
    }
}
```

### Conversation Memory

Intelligent conversation state management:

```rust
pub struct ConversationMemory {
    pub short_term: VecDeque<Exchange>,  // Last N exchanges
    pub long_term: SummaryStore,         // Compressed summaries
    pub context_graph: Graph<String>,     // Relationships between concepts
}

pub struct Exchange {
    pub prompt: String,
    pub response: String,
    pub metadata: ExchangeMetadata,
}

pub struct ExchangeMetadata {
    pub timestamp: DateTime<Utc>,
    pub tokens_used: usize,
    pub spec_name: String,
    pub iteration: u32,
    pub success: bool,
}
```

### Model Selection

Dynamic model selection based on task:

```yaml
# Model configuration
models:
  default: "claude-3-sonnet"
  
  tasks:
    planning:
      model: "claude-3-opus"
      reason: "Complex reasoning required"
    
    implementation:
      model: "claude-3-sonnet"
      reason: "Good balance of capability and speed"
    
    review:
      model: "claude-3-haiku"
      reason: "Fast iteration for simple reviews"
    
    debug:
      model: "claude-3-opus"
      reason: "Deep analysis needed"
```

### Response Caching

Intelligent caching system:

```rust
pub struct ResponseCache {
    cache_dir: PathBuf,
    max_age: Duration,
    max_size: usize,
}

impl ResponseCache {
    pub fn get_cached(&self, prompt_hash: &str) -> Option<CachedResponse> {
        // Check if response exists and is fresh
        // Validate cache integrity
        // Return cached response with metadata
    }
    
    pub fn should_cache(&self, response: &Response) -> bool {
        // Cache successful responses
        // Cache expensive operations
        // Don't cache interactive or time-sensitive queries
    }
}
```