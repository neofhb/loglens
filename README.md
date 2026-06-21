# LogLens 本地日志分析工具

LogLens 是一个使用 Rust 编写的命令行日志分析工具，支持读取本地普通日志、JSONL 和 CSV 文件，并提供扫描、过滤、统计、容错诊断和报告导出功能。


## 功能特性

- 支持普通日志、JSONL、CSV 三种输入格式。
- 支持根据文件扩展名自动识别输入格式。
- 支持单文件输入，也支持目录递归扫描。
- 支持按日志等级、最小日志等级、关键词、正则表达式、时间范围过滤。
- 支持按结构化字段过滤，例如 `--field user=alice`。
- JSONL 或 CSV 中出现坏行时，默认保留合法记录并收集解析诊断。
- 支持 `--strict` 严格模式，遇到第一条坏行时直接返回错误。
- 支持等级分布、源文件分布、字段分组、字段 Top N 和数据质量统计。
- 支持导出 Markdown、JSON 和 CSV 报告。
- 包含单元测试，覆盖解析、过滤、统计、报告、严格模式和目录扫描等关键功能。

## 编译方式

在项目根目录运行：

```bash
cargo build
```

## 运行示例

扫描普通日志文件：

```bash
cargo run -- scan examples/app.log
```

扫描 JSONL 文件并显示解析诊断：

```bash
cargo run -- scan examples/app.jsonl
```

筛选错误日志：

```bash
cargo run -- filter examples/app.log --level error
```

按关键词搜索：

```bash
cargo run -- filter examples/app.jsonl --keyword timeout
```

按最小日志等级和结构化字段过滤：

```bash
cargo run -- filter examples/app.jsonl --min-level warn --field user=bob
```

按日志等级统计：

```bash
cargo run -- stats examples/app.csv --group-by level
```

按结构化字段分组统计：

```bash
cargo run -- stats examples/app.jsonl --group-by field --field-name user
```

统计指定字段的 Top N：

```bash
cargo run -- stats examples/app.jsonl --group-by field --field-name user --top-field service
```

生成 Markdown 报告：

```bash
cargo run -- report examples/app.log --output report.md
```

生成 JSON 报告：

```bash
cargo run -- report examples/app.jsonl --output report.json
```

生成 CSV 报告：

```bash
cargo run -- report examples/app.jsonl --output report.csv
```

## 命令说明

基本命令格式如下：

```bash
loglens scan <path> --format auto --strict
loglens filter <path> --level error --keyword timeout
loglens filter <path> --min-level warn --field user=bob
loglens stats <path> --group-by field --field-name user --top-field service
loglens report <path> --output report.md --sample-limit 20
```

其中 `<path>` 可以是单个文件，也可以是目录。输入目录时，程序会递归扫描其中的文件。

支持的输入格式：

- `auto`
- `plain-log`
- `jsonl`
- `csv`

支持的统计分组方式：

- `level`：按日志等级分组。
- `source`：按源文件分组。
- `hour`：按小时分组。
- `field`：按指定结构化字段分组。

使用 `--group-by field` 时，需要同时传入 `--field-name <字段名>`。`--field key=value` 专门用于过滤记录。

报告格式由输出文件扩展名决定：

- `.md` 或其他扩展名：Markdown 报告。
- `.json`：JSON 报告。
- `.csv`：标准化日志记录 CSV。

## 项目结构

```text
src/
├── main.rs          程序入口
├── lib.rs           模块导出
├── cli.rs           命令行参数与子命令调度
├── model.rs         核心数据结构和枚举
├── parser.rs        普通日志、JSONL、CSV 解析器
├── input.rs         文件/目录加载和 strict 模式
├── filter.rs        过滤条件实现
├── stats.rs         统计聚合逻辑
├── diagnostics.rs   数据质量诊断
├── report.rs        Markdown/JSON/CSV 报告导出
└── error.rs         统一错误类型
```

示例数据位于 `examples/`：

- `examples/app.log`
- `examples/app.jsonl`
- `examples/app.csv`
- `examples/real_server.log`：公开项目中的真实服务日志样例，用于展示真实日志统计。

## Rust 特性体现

- 使用 `struct` 表示日志记录、数据集、解析问题、统计摘要和诊断结果。
- 使用 `enum` 表示日志等级、输入格式、分组方式和错误类型。
- 使用 `trait RecordParser` 抽象不同格式的解析器。
- 使用 `trait Aggregator<T>` 展示泛型聚合接口。
- 使用 `Result<T, LoglensError>` 进行结构化错误处理。
- 使用所有权和借用区分数据加载、过滤、统计和报告阶段的数据流。
- 使用单元测试覆盖核心功能。

## 工程检查

提交前建议运行：

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

当前项目已通过上述检查。

