# crate-stats
Crate Stats and Download Crates tools for W&amp;M CSCI 680 Course (Fall 2022).

With Rust installed on the system, Crate Stats can be run via the following command:

```
cargo run --bin crate-stats
```

Similarly, the Download Crates tool can be run via the following command:

```
cargo run --bin download_crates
```

With Julia installed on the system, the statistics code can be opened in browser with the following command:

```
julia -e "import Pluto; Pluto.run();"
```

One the locally hosted Pluto webpage is open, select either [statistics-v1](/statistics-v1.jl) or [statistics-v2](/statistics-v2.jl) to see the statistics generated for our first and second presentation updates as well as the paper.

A compressed copy of the Postgres database used to generate the above statistics is available via the following Google Drive link:

https://drive.google.com/drive/folders/1KX7S66DW3K5JD-1t9B5gEwEl9BZa-Wwl?usp=sharing


Once downloaded, the compressed database can be reloaded via the following commands:

```
createdb dbname
gunzip -c filename.gz | psql dbname
```

See https://www.postgresql.org/docs/8.1/backup.html#BACKUP-DUMP-LARGE for details.

This project was developed by Collin MacDonald and Sam Sartor.
