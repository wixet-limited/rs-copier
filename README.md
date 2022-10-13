# Why?
There are a plenty amount of good programs out there that do this task great. So why another one? Because there is a single case where where I could not find any solution good enough and this is why I made this application. The case is when you want to move a bunch of small files distributed in many
directories from one disk to another.

In my case I have several nodes collecting data with small disks (5TB) which are generating like 800 MB daily distributed in 800 files in 50 directories.
Once the small disk is more or less full, I dump all the contents to a bigger storage (where it will be processed). This task implies moving millions of files. Using `rsync` or other traditional tools takes a long time. Basically because they copy the files one by one and trying
to make rsync concurrent requires a lot of "tricks" using tools like parallel or xargs, apart from preparing a correct strategy
that allows an effective use of these tools. It was very "complicated" and I wanted something easy to use but powerful.

This application just does this concurrent stuff.

# Copier

This program `copies` the files in an `asynchronous` way. Every directory is procesed in a different `tokio` task. It uses a task pool to control
the maximum concurrency. Basically the program discovers new directories and spawns more tasks as soon as it find new directories. You have
to choose this value wisely because more concurrency does not mean more speed and actually a big value may make your disk transfers slower. Asynchronous soluions are a game changer in some situations but they are not a silver bullet.

I found out that using this solution I can reach the maximum throughput that the disks can give but
you have to find the best value for your disks by trying different values while measuring it with tools like `iotop`.

If you have a few directories with huge files, this program will never out perform `rsync` and it could be even slower. Remember that `asyc` costs, 
and this overhead does not provide any benefit in this case.


# When use this program
* Huge amount of files distributed in many directories
* Small files (up to a few megabytes)

# Whe not to use this progarm
* Big files
* Single o a few directories


# Using the program

Only two parameteres are required: source and destination. Apart from that, you can specify if you want to remove the source (move the files) and the concurrency level. For example:

```
--source data_origin --destination data_destination --delete-source true --concurrency 20
```

But you can always run with `--help` to get more details

# Lacking functionalities

Metrics, progress bar and these kind of fancy things are not implemented. 