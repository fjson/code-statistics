## 代码统计工具

### 开发环境

- rustc: 1.71.0
- system: windows,linux,mac
- runtime: windows,linux,mac

### 编译

#### 安装docker

[docker文档](https://docs.docker.com/get-docker/)

#### 安装cross

[cross 文档](https://github.com/cross-rs/cross)

```bash
cargo install cross --git https://github.com/cross-rs/cross
```
#### 编译至目标平台

```bash
cross build --target x86_64-unknown-linux-musl --release
```


#### 用例

```bash
code-statistics -i git项目路径 -s 开始时间 -e 结束时间 [--author 根据提交人显示]
```


