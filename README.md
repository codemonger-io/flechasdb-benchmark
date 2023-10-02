# flechasdb Benchmark

Benchmark for [`flechasdb`](https://github.com/codemonger-io/flechasdb).

`flechasdb` is the core library of the FlechasDB system which provides a lightweight vector database.
This benchmark evaluates [`flechasdb`](https://github.com/codemonger-io/flechasdb) with the well-known [SIFT 1M](http://corpus-texmex.irisa.fr/#matlab) dataset which is used in [this article](https://www.pinecone.io/learn/series/faiss/product-quantization/).
It measures the following:
- One-shot time to build a database from `sift/sift_base.fvecs`
    - Number of vectors: 1,000,000
    - Number of partitions: 2048
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
    - Metrics for query time in milliseconds
    - Metrics for recall (%)
        - Recall is defined as the percentage of k-NN vectors that are in the reference set made by flat k-NN search\*.
    - Parameters
        - k-nearest-neighbors where k=100
        - nprobe
            - 1
            - 10
            - 20
            - 50
            - 2048

\* flat k-NN search calculates distances between a query vector and all the vectors in the dataset and selects k vectors of shortest distances.

## Results

Here are results on my machine (Apple M1 Pro, 32GB RAM, 1TB SSD).

### Results: building a database

It took about 6,015 seconds to build and 4.5 seconds to save the database.
Disk usage was about 77.5 MB, where the database header file was about 185 KB.

### Results: query time (ms)

| sync/async/flat | nprobe | μ ± σ | med | Q1 | Q3 | min | max |
| --------------- | ------ | ----- | --- | --- | --- | -- | -- |
| sync | 1 | 0.548 ± 0.631 | 0.455 | 0.411 | 0.547 | 0.275 | 52.206 |
| async | 1 | 0.784 ± 0.446 | 0.666 | 0.619 | 0.769 | 0.459 | 31.456 |
| sync | 10 | 3.131 ± 1.303 | 2.844 | 2.523 | 3.274 | 1.747 | 32.443 |
| async | 10 | 1.327 ± 0.876 | 1.205 | 1.112 | 1.321 | 0.798 | 52.946
| sync | 20 | 4.735 ± 1.022 | 4.606 | 4.113 | 5.182 | 2.821 | 40.918 |
| async | 20 | 1.771 ± 0.877 | 1.641 | 1.506 | 1.813 | 1.105 | 33.117 |
| sync | 50 | 12.756 ± 1.843 | 12.587 | 11.338 | 14.012 | 8.503 | 43.592 |
| async | 50 | 3.062 ± 1.110 | 2.936 | 2.713 | 3.187 | 2.109 | 55.332 |
| sync | 2048 | 395.638 ± 7.980 | 395.591 | 391.266 | 399.507 | 380.793 | 969.689 |
| async | 2048 | 82.381 ± 4.814 | 82.006 | 81.325 | 83.001 | 79.716 | 477.053 |
| flat | n/a | 72.878 ± 1.045 | 72.679 | 72.137 | 73.351 | 71.342 | 94.256 |

### Results: recall (%)

| nprobe | μ ± σ | med | Q1 | Q3 | min | max |
| ------ | ----- | --- | --- | --- | -- | -- |
| 1 | 22.6 ± 15.2 | 19.0 | 11.0 | 30.0 | 0.0 | 90.0 |
| 10 | 45.5 ± 11.9 | 45.0 | 37.0 | 53.0 | 8.0 | 90.0 |
| 20 | 47.9 ± 10.7 | 47.0 | 40.0 | 55.0 | 17.0 | 90.0
| 50 | 48.9 ± 10.0 | 48.0 | 42.0 | 55.0 | 17.0 | 90.0 |
| 2048 | 49.1 ± 9.8 | 48.0 | 42.0 | 55.0 | 18.0 | 90.0 |

Sync or async does not matter to recalls.

## Running the benchmark yourself

### Preparing the SIFT 1M dataset

Download the ANN_SIFT1M dataset from [here](http://corpus-texmex.irisa.fr/#matlab) and extract it.
The following sections suppose that the dataset is extracted in `sift` folder and use the following files:
- `sift/sift_base.fvecs`
- `sift/sift_query.fvecs`

### Building a database

The following command will read the vector set from `sift/sift_base.fvecs`, build a database, and save it in `database` folder with the default parameters:

```sh
mkdir database
cargo run --release -- build sift/sift_base.fvecs database
```

You will find a file ending with `.binpb` in `database` folder, which is the database header file.
Here is an example of `database` folder contents:
- `5xm2LZluq4xGfGAMwtIukv3v4sfKSqrde0hRVRIJnlQ.binpb`
- `attributes/`
- `codebooks/`
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

The following command will load a database in `database` folder and search k-nearest neighbors for a query vector randomly chosen from `sift/sift_query.fvecs`:

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

The following command will load a database in `database` folder and benchmark the performance with a query vector set `sift/sift_query.fvecs` with default parameters:

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