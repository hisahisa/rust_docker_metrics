# rust_docker_metrics

## ビルドと起動方法

- 以下のコマンドを叩いてビルド&起動する(dockerが動作していることが前提)

```asm
yogo-MacBook:rust_metrics $ cargo build
Compiling rust_metrics v0.1.0 (/Users/CLionProjects/rust_metrics)
Finished dev [unoptimized + debuginfo] target(s) in 16.26s

yogo-MacBook:rust_metrics $ ls -al target/debug/rust_metrics
-rwxr-xr-x  1  staff  3415664 Mar  5 13:48 target/debug/rust_metrics

yogo-MacBook:rust_metrics $ chmod +x target/debug/rust_metrics
yogo-MacBook:rust_metrics $ ./target/debug/rust_metrics
```

- 以下のurlでアクセス
  - http://127.0.0.1:7878/chart.html
  - 10秒間隔でdocker上で動作するメトリクスを表示します。
  - 360回 x 10秒間隔で取得するので1時間分の動作状況を表示します。

  - ![img.png](img.png)
  - 

## 課題
  - 一旦初回作品なので、まずは動くものをということを目標に作りました。課題としてい以下があげれれるかと思っています。
    - mainファイルだけで組んでいる。構造体とか別ファイルで管理したいところ。
    - エラーハンドリングはもう少し手を加えられそう。
    - テストコードもあればなお理想。