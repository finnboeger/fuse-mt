# ultrastar-fs

*ultrastar-fs* is a tool to allow [UltraStar Deluxe](https://github.com/UltraStar-Deluxe/USDX) to start up faster. It's mainly useful when dealing with a very large song collection, a slow storage medium or both. In combination with e.g. sshfs or davfs it enables you to run ultrastar using remote storage without waiting for an eternity on startup.

*ultrastar-fs* works by building a cache of all relevant data in advance. This consists of 3 parts:
- the attributes and paths of all existing files and directories to allow for faster directory traversal, 
- all .txt files and
- a cover.db containing the metadata and thumbnails for all cover images. 

Using *ultrastar-fs* is as simple as 
1. Building the cache. (It is best to do this on the remote system in case remote storage is being used.)

   `cargo run build <path to songdirectory>`

   this creates a `cache.zip` that you can then use in the future.
2. Mounting ultrastar-fs.

   `cargo run mount <path to source> <mount point> -i <path to usdx config dir>`
   
   This will wrap the `source` and expose it at the provided mount point. All calls to that mount point will be passed through ultrastar-fs and sped up using the cache.

More information can be gathered by running `cargo run help`
