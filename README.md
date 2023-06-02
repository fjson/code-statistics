## 版本管理工具

### 开发环境

- rustc: 1.69.0
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


