# flechasdb Benchmark

Benchmark for [`flechasdb`](https://github.com/codemonger-io/flechasdb).

`flechasdb` is the core library of the FlechasDB system which provides a lightweight vector database.
This benchmark evaluates [`flechasdb`](https://github.com/codemonger-io/flechasdb) with the publicly available [SIFT 1M](http://corpus-texmex.irisa.fr/) dataset which is used in [this article](https://www.pinecone.io/learn/series/faiss/product-quantization/).
It measures the following:
- One-shot time to build a database from `sift/sift_base.fvecs`
    - Number of vectors: 1,000,000
    - Number of partitions: 2,048
    - Number of codes: 256
- Database disk usage
- Statistics over 10,000 queries from `sift/sift_query.fvecs` (synchronous / asynchronous)
    - Metrics
        - Mean (μ)
        - Standard deviation (σ)
        - Median (med)
        - 1st quartile (Q1)
        - 3rd quartile (Q3)
        - Minimum (min)
        - Maximum (max)
    - Above metrics for query time in milliseconds
    - Above metrics for recall (%)
        - Recall is defined as the percentage of k-nearest neighbor (k-NN) vectors that are in the reference set made by flat k-NN search\*.
    - Parameters
        - k(-NN): 100
        - nprobe:
            - 1
            - 10
            - 20
            - 50
            - 2048

\* flat k-NN search calculates distances between a query vector and all the vectors in the dataset and selects k vectors of shortest distances.

## Results

Here are results on my machine (Apple M1 Pro, 32GB RAM, 1TB SSD).
`flechasdb` version was `0.1.0`.

### Results: building a database

It took about 5,880 seconds to build and 5.7 seconds to save the database.
Disk usage was about 61.1 MB, where the database header file was about 139 KB.

### Results: query time (ms)

| sync/async/flat | nprobe | μ ± σ | med | Q1 | Q3 | min | max |
| --------------- | ------ | ----- | --- | --- | --- | -- | -- |
| sync | 1 | 0.566 ± 0.431 | 0.453 | 0.409 | 0.552 | 0.251 | 29.829 |
| async | 1 | 0.818 ± 0.711 | 0.645 | 0.601 | 0.747 | 0.458 | 53.028 |
| sync | 10 | 2.636 ± 0.788 | 2.495 | 2.234 | 2.823 | 1.449 | 32.969 |
| async | 10 | 1.396 ± 1.119 | 1.210 | 1.117 | 1.324 | 0.814 | 34.000 |
| sync | 20 | 4.946 ± 1.557 | 4.677 | 4.186 | 5.280 | 2.974 | 36.120 |
| async | 20 | 1.884 ± 1.333 | 1.649 | 1.523 | 1.798 | 1.134 | 44.262 |
| sync | 50 | 11.342 ± 2.099 | 11.107 | 10.051 | 12.299 | 7.743 | 44.177 |
| async | 50 | 3.131 ± 1.634 | 2.897 | 2.696 | 3.133 | 2.150 | 62.028 |
| sync | 2048 | 398.281 ± 5.754 | 398.369 | 394.381 | 401.831 | 384.762 | 710.972 |
| async | 2048 | 84.778 ± 6.885 | 84.447 | 83.744 | 85.170 | 81.530 | 518.380 |
| flat | n/a | 72.878 ± 1.045 | 72.679 | 72.137 | 73.351 | 71.342 | 94.256 |

### Results: recall (%)

| nprobe | μ ± σ | med | Q1 | Q3 | min | max |
| ------ | ----- | --- | --- | --- | -- | -- |
| 1 | 22.6 ± 15.3 | 19.0 | 11.0 | 30.0 | 0.0 | 89.0 |
| 10 | 45.6 ± 11.9 | 45.0 | 37.0 | 53.0 | 7.0 | 89.0 |
| 20 | 47.9 ± 10.7 | 47.0 | 40.0 | 55.0 | 10.0 | 89.0 |
| 50 | 48.9 ± 10.0 | 48.0 | 42.0 | 55.0 | 15.0 | 89.0 |
| 2048 | 49.1 ± 9.8 | 48.0 | 42.0 | 55.0 | 19.0 | 89.0 |

Sync or async does not matter to recalls.

## Running the benchmark yourself

### Preparing the SIFT 1M dataset

Download the ANN_SIFT1M dataset from [here](http://corpus-texmex.irisa.fr/#matlab) and extract it.
The following sections suppose that the dataset is extracted in `sift` folder and use the following files:
- `sift/sift_base.fvecs`
- `sift/sift_query.fvecs`

### Building a database

You have to [prepare the SIFT 1M dataset](#preparing-the-sift-1m-dataset) first.

The following command will read the vector set from `sift/sift_base.fvecs`, build a database, and save it in `database` folder with the default parameters:

```sh
cargo run --release -- build sift/sift_base.fvecs database
```

You will find a file ending with `.binpb` in `database` folder, which is the database header file.
Here is an example of `database` folder contents:
- `attributes/`
- `codebooks/`
- `jsSHz9ujU9HSpXHxjfwBAm0TiSUZq8MvYnYfF_1ZOXM.binpb` &leftarrow; database header file
- `partitions/`

Passing `--help` flag to the command will show the usage:

```
Builds the database

Usage: flechasdb-benchmark build [OPTIONS] <DATASET_PATH> <OUTPUT_PATH>

Arguments:
  <DATASET_PATH>  Path to the dataset (*.fvecs file)
  <OUTPUT_PATH>   Path to the folder where to save the database

Options:
  -p, --num-partitions <NUM_PARTITIONS>  Number of partitions [default: 2048]
  -d, --num-divisions <NUM_DIVISIONS>    Number of subvector divisions [default: 8]
  -c, --num-codes <NUM_CODES>            Number of clusters (codes) [default: 256]
  -h, --help                             Print help
```

### Testing a single query vector

You have to [build the database](#building-a-database) first.

The following command will load a database in `database` folder and search k-NN for a query vector randomly chosen from `sift/sift_query.fvecs`:

```sh
cargo run --release -- query sift/sift_base.fvecs database/*.binpb sift/sift_query.fvecs
```

The first parameter `sift/sift_base.fvecs` is used to evaluate recalls of the search results.

Passing `--help` flag to the command will show the usage:

```
Queries the database with a single query vector

Usage: flechasdb-benchmark query [OPTIONS] <DATASET_PATH> <DATABASE_PATH> <QUERIES_PATH>

Arguments:
  <DATASET_PATH>   Path to the dataset (*.fvecs file)
  <DATABASE_PATH>  Path to the database file
  <QUERIES_PATH>   Path to the query vectorset (*.fvecs file)

Options:
  -q, --query-index <QUERY_INDEX>  Index of the query to evaluate. Randomly chosen if omitted
  -k, --k <K>                      Number of best matches (k-nearest neighbors) to return [default: 100]
  -p, --nprobe <NPROBE>            Number of partitions to search in [default: 10]
  -h, --help                       Print help
```

### Benchmarking with a query vector set

You have to [build the database](#building-a-database) first.

The following command will load a database in `database` folder and measure the performance with a query vector set `sift/sift_query.fvecs` with default parameters:

```sh
cargo run --release -- batch sift/sift_base.fvecs database/*.binpb sift/sift_query.fvecs
```

If `--async` flag is provided, it will test asynchronous queries.

```sh
cargo run --release -- batch sift/sift_base.fvecs database/*.binpb sift/sift_query.fvecs --async
```

Passing `--help` flag to the command will show the usage:

```
Queries the database with every query vector

Usage: flechasdb-benchmark batch [OPTIONS] <DATASET_PATH> <DATABASE_PATH> <QUERIES_PATH>

Arguments:
  <DATASET_PATH>   Path to the dataset (*.fvecs file)
  <DATABASE_PATH>  Path to the database file
  <QUERIES_PATH>   Path to the query vectorset (*.fvecs file)

Options:
  -k, --k <K>                    Number of best matches (k-nearest neighbors) to return [default: 100]
  -p, --nprobe <NPROBE>          Number of partitions to search in [default: 10]
  -s, --stats-path <STATS_PATH>  Output path of the statistics
  -l, --limit <LIMIT>            Limits the number of queries
  -a, --async                    Whether asynchronously executed
  -h, --help                     Print help
```