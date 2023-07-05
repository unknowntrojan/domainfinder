# domainfinder

This small tool operates on wordlists to find "domain hacks", words where the last few characters correspond to a TLD, making the end of the domain useful instead of bloat.

For example, when searching for ".rs" domain hacks, we would find things such as "refrigerato.rs".

It queries Cloudflare's DNS server for each combination and filters out every domain that is already registered. It then spits out a simple text file with each line containing a domain that is still available.

To change the TLD to search, simply change line 11 in [src/main.rs](src/main.rs).

```rs
const TLD: &str = "rs";
```

There's two included word lists. One is from [here](https://github.com/first20hours/google-10000-english), the other I don't remember where I got it from.
