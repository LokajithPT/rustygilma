# Gilma

**G**it **L**ike **M**emory **A**ccess - Because your files deserve better than being scattered across your hard drive like lost socks.

Gilma is a ridiculously simple file synchronization tool that speaks Tamil but thinks in Rust. It's like having a personal butler for your directories, except this butler doesn't judge your questionable file naming conventions.

## What It Does

Gilma consists of two parts that work together in perfect harmony:

1. **gilma-server** - The overworked server that sits on port 8080 and patiently waits for your files
2. **gilma** - The client that actually does the heavy lifting while you sip coffee

## Installation

```bash
# Build both the server and client
cargo build --release

# The client binary will be named 'gilma'
# The server binary will be named 'gilma-server'
```

## Usage

### Step 1: Start the Server

```bash
./gilma-server
# Server will start listening on 127.0.0.1:8080
# Creates a 'gilma_storage' directory because even servers need a home
```

### Step 2: Use the Client

The client commands are in Tamil because why not make file synchronization culturally enriching?

#### Push your entire directory to the server
```bash
gilma vechuko
# "Vechuko" means "put it" - creative, I know
```

#### List all folders stored on the server
```bash
gilma kaami
# "Kaami" means "list" - because staring at directory listings is a valid hobby
```

#### Pull a specific folder from the server
```bash
gilma vangiko folder_name
# "Vangiko" means "catch it" - like catching files, but with more enthusiasm
```

#### Sync only changed files (the smart option)
```bash
gilma sync
# Compares timestamps and only transfers what's changed
# Because your time is more valuable than bandwidth
```

## Features That Might Impress You

- **Delta Sync**: Only transfers files that have actually changed. Your network bill will thank you.
- **Tamil Commands**: Learn a language while synchronizing files. Multitasking at its finest.
- **Colorful Output**: Because monochrome terminals are so 1990s.
- **Error Handling**: Gracefully handles everything except your existential dread.
- **Ignores Junk**: Automatically skips `target/` and `.git/` directories. We're not savages.

## Protocol

The communication protocol is basically:
1. Client sends commands in ALL CAPS because shouting gets attention
2. Server responds with files and timestamps
3. Everyone pretends this is more sophisticated than FTP

## Storage

All files are stored in `gilma_storage/` with the original directory structure preserved. It's like a museum for your code, except admission is free and the exhibits change frequently.

## Limitations

- Runs on localhost only because sharing is caring but not that much caring
- No authentication - assumes you trust yourself (questionable, I know)
- No compression - because modern networks are fast enough
- Tamil commands might confuse your non-Tamil-speaking colleagues (feature, not bug)

## Contributing

Feel free to contribute! Just remember:
- The Tamil command names are non-negotiable
- More colors are always better
- If it works, don't question it

## License

This project is licensed under the "Do Whatever You Want, I'm Not Your Parent" license.

---

*P.S. If you're wondering why it's called "Gilma" - sometimes names just happen. Accept it and move on with your life.*