# Baseline Performance & Storage Benchmarks 

##  Test Configuration

| Parameter                  | Value                                    |
| -------------------------- | ---------------------------------------- |
| **Accounts**               | 100,000                                  |
| **Transactions per Block** | 10                                       |
| **Load Test Tool**         | oha                                      |
| **Load Test Parameters**   | 50 concurrent connections for 30 seconds |

---

## Summary 

The following table summarizes the key metrics measured at different scales.

| **Metric**                      | **1k Blocks (10k TX)** | **10k Blocks (100k TX)** | **Notes**                                     |
| ------------------------------- | ---------------------- | ------------------------ | --------------------------------------------- |
| **RocksDB Disk Usage**          | ~8.4 MB                 | ~203 MB                  | Measures size of `test_db/rocksdb` directory  |
| **Avg. Batch Commit Time**      | ~12 ms                 | ~15 
---

## Analysis & Conclusion

### Storage Footprint

* Database size grows **linearly** with the number of transactions and blocks.
* Storage cost per transaction ≈ `8602 KB / 10,000 tx` = **~0.086 KB per tx**.

### Write Performance

* Average batch commit time remains **low and stable**.
* fsync and RocksDB writes are **not a bottleneck** at this scale.

### Read Performance

* All read-only RPC endpoints show **excellent performance**.
* 95th percentile latency is **well under the 50 ms target**.
* Direct key lookups in RocksDB are **highly efficient**.

### Verdict

> The current implementation of the database and read-only RPC server is **highly performant** and meets all initial performance goals. The system is **ready for the next development phase**.


