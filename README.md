# Redis CLI with Smart Completion

这是一个基于 Rust 开发的 Redis 命令行客户端，具有智能命令补全功能。

## 功能特性

### 🎯 智能命令补全
- **Tab 补全**: 输入部分命令后按 Tab 键自动补全
- **上下文感知**: 根据命令类型提供相应的参数补全
- **Key 补全**: 自动获取现有 key 并提供补全建议
- **参数提示**: 为不同命令提供智能参数建议

### 🚀 支持的命令类型

#### 基础命令
- `GET <key>` - 获取键值
- `SET <key> <value>` - 设置键值  
- `DEL <key>` - 删除键
- `KEYS <pattern>` - 查找匹配的键
- `EXISTS <key>` - 检查键是否存在
- `TYPE <key>` - 获取键类型
- `TTL <key>` - 获取键的生存时间
- `EXPIRE <key> <seconds>` - 设置键过期时间

#### Hash 命令
- `HGET <key> <field>` - 获取哈希字段值
- `HSET <key> <field> <value>` - 设置哈希字段
- `HDEL <key> <field>` - 删除哈希字段
- `HGETALL <key>` - 获取所有哈希字段

#### List 命令
- `LPUSH <key> <value>` - 从左侧插入列表元素
- `RPUSH <key> <value>` - 从右侧插入列表元素
- `LPOP <key>` - 从左侧弹出元素
- `RPOP <key>` - 从右侧弹出元素
- `LLEN <key>` - 获取列表长度

#### Set 命令
- `SADD <key> <member>` - 添加集合成员
- `SREM <key> <member>` - 删除集合成员
- `SMEMBERS <key>` - 获取所有集合成员
- `SCARD <key>` - 获取集合成员数量

#### Sorted Set 命令
- `ZADD <key> <score> <member>` - 添加有序集合成员
- `ZREM <key> <member>` - 删除有序集合成员
- `ZRANGE <key> <start> <stop>` - 获取成员范围
- `ZCARD <key>` - 获取成员数量

#### 服务器命令
- `INFO [section]` - 获取服务器信息
- `CONFIG GET <parameter>` - 获取配置参数
- `CONFIG SET <parameter> <value>` - 设置配置参数
- `PING` - 测试连接
- `FLUSHDB` - 清空当前数据库
- `FLUSHALL` - 清空所有数据库

## 智能补全示例

### 1. 命令补全
```
> GE[TAB]
GET

> SE[TAB]  
SET
```

### 2. Key 补全
```
> GET [TAB]
user:123
session:456
cache:data
```

### 3. 参数补全
```
> SET user:123 [TAB]
"value"
123
true
false

> SET user:123 "hello" [TAB]
EX  PX  NX  XX
```

### 4. 特殊命令补全
```
> CONFIG [TAB]
GET  SET  RESETSTAT

> INFO [TAB]
""  server  clients  memory  persistence  stats  replication  cpu  commandstats  cluster  keyspace

> KEYS [TAB]
*  user:*  session:*  cache:*
```

## 使用方法

### 1. 编译项目
```bash
cargo build --release
```

### 2. 运行客户端
```bash
# 连接本地 Redis (默认端口 6379)
./target/release/rredis-cli.exe localhost

# 指定端口
./target/release/rredis-cli.exe localhost 6379

# 使用密码连接
./target/release/rredis-cli.exe localhost 6379 mypassword
```

### 3. 在交互界面中使用
- 输入命令时按 `Tab` 键进行补全
- 使用 `help` 命令查看所有可用命令
- 使用 `quit` 或 `exit` 退出
- 使用上下箭头键浏览历史命令

## 技术实现

### 核心组件

1. **CommandCache**: Redis 命令信息缓存
   - 通过 `COMMAND` 获取命令列表
   - 通过 `COMMAND DOC` 获取详细文档
   - 定期更新 key 缓存

2. **SmartCompleter**: 智能补全器
   - 解析命令行上下文
   - 根据命令类型提供相应补全
   - 实现 rustyline 的 Completer trait

3. **RedisClient**: Redis 客户端
   - 支持 RESP3 协议
   - 异步命令执行
   - 自动重连机制

### 补全策略

1. **命令补全**: 匹配所有可用 Redis 命令
2. **Key 补全**: 基于 `KEYS *` 命令获取现有 key
3. **参数补全**: 根据命令的 arity 和类型提供智能建议
4. **值补全**: 为特定命令提供常用值建议

## 依赖项

- `rustyline`: 命令行编辑和补全
- `anyhow`: 错误处理
- `num-bigint`: 大整数支持

## 性能优化

1. **缓存策略**: Key 缓存每 30 秒更新一次
2. **后台更新**: 使用独立线程获取命令文档
3. **增量补全**: 只补全匹配当前前缀的项目
4. **内存管理**: 使用 Arc<Mutex<>> 共享缓存

## 扩展功能

计划添加的功能：
- 语法高亮
- 命令历史持久化
- 配置文件支持
- 连接池管理
- SSL/TLS 支持

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License