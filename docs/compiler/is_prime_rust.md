# Writing a MidenVM Program In Rust

*Using the Miden compiler to write programs in Rust and generate a proof of computation using the MidenVM CLI*

## Overview

In this guide, we will write a simple Rust program that checks whether an integer is prime. We will compile the Rust program into a Miden package and run it in the Miden VM. We will also see how to use the Miden CLI to generate a STARK proof that the computation was performed correctly.

## What we'll cover

- Writing basic programs in Rust using the Miden compiler.
- Running programs in the Miden VM.
- Generating a proof of compuation for the `is_prime` program
- Verifying the STARK proof of the `is_prime` program execution

## Limitations and Important Considerations

Please note these current limitations of the Miden compiler:
- **No Floating Point Support:** Only integer arithmetic is supported (e.g., `u32`, `u64`, etc.).
- **No Standard Library:** Programs must be written with `#![no_std]`, limiting you to core library functionality.
- **Entrypoint Constraints:** The `entrypoint` function can accept at most **16 inputs** on the stack and produces a single `u32` output.

## Step 1: Installing the Miden Compiler

Clone the repository and install the compiler:
```bash
git clone https://github.com/0xpolygonmiden/compiler
cd compiler
git checkout next
```

Then install the Miden compiler and cargo-miden extension:
```bash
cargo make build 
```

## Step 2: Writing the Rust Program

Outside of the compiler repository, create a new Miden project:
```bash
cargo miden new is_prime
cd is_prime
```

Add the following Rust code to `is_prime/src/lib.rs`. This code checks whether a number is prime:
```rust
#![no_std]

// Custom panic handler since we don't have the standard library.
#[cfg(not(test))]
#[panic_handler]
fn my_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Returns true if the integer is prime.
fn is_prime(n: u32) -> bool {
    if n <= 1 {
        return false;
    }
    if n <= 3 {
        return true;
    }
    if n % 2 == 0 || n % 3 == 0 {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 {
            return false;
        }
        i += 6;
    }
    true
}

/// The entry point to the Miden program.
#[no_mangle]
fn entrypoint(n: u32) -> bool {
    is_prime(n)
}
```

Add this code into your project's `src/lib.rs` file.

Next, create an `is_prime/inputs.toml` file:
```toml
[inputs]
stack = [2147482583]
```

This file sets the value that will be passed into our `entrypoint` function when the program runs.

## Step 3: Running the Program in the Miden VM

Compile your program with:
```bash
cargo miden build --release
```

Run your compiled Miden assembly program using:
```bash
midenc run target/miden/release/is_prime.masp --inputs inputs.toml
```

The output will look like this:
```
Run program: target/miden/release/is_prime.masp
-------------------------------------------------------------------------------
Executed program with hash 0x79689b17ab6286cfde4651ef1f675cab19ad4efd9defd2c43001a06e7cbd8c40 in 2 seconds
Output: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
VM cycles: 2039234 extended to 2097152 steps (2% padding).
├── Stack rows: 1668454
├── Range checker rows: 61329
└── Chiplets rows: 2039234
    ├── Hash chiplet rows: 1792040
    ├── Bitwise chiplet rows: 247192
    ├── Memory chiplet rows: 1
    └── Kernel ROM rows: 0
```

The program returns `1` if the integer passed to the `is_prime` function is prime and `0` if it is not.

## Step 4: Generating a zk proof of the `is_prime` program execution

First install the Miden CLI by cloning the Miden VM repository and checking out the `next` branch:
```bash
git clone git@github.com:0xPolygonMiden/miden-vm.git
cd miden-vm
git checkout next
```

Build and install the Miden VM CLI:
```
cd miden
cargo install --path . --features concurrent,executable
```

After installation is complete, return to the `is_prime` directory.

The current input file format for the Miden VM differs slightly from that of the compiler. This means we need to create an `is_prime.inputs` file at the root of the `is_prime` directory:
```json
{
    "operand_stack": ["2147482583"]
}
```

Now, using the Miden VM CLI tool, we can prove our program by running the following:
```
miden prove target/miden/release/is_prime.masp -i is_prime.inputs
```

The output should look like this:

```
===============================================================================
Prove program: target/miden/release/is_prime.masp
-------------------------------------------------------------------------------
Proving program with hash 79689b17ab6286cfde4651ef1f675cab19ad4efd9defd2c43001a06e7cbd8c40...
Program proved in 85558 ms
Output: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
```

To verify the proof generated in the previous step, run the following:
```
miden verify -p target/miden/release/is_prime.proof -i is_prime.inputs -x 79689b17ab6286cfde4651ef1f675cab19ad4efd9defd2c43001a06e7cbd8c40
```

The output should look like this:
```
===============================================================================
Verifying proof: target/miden/release/is_prime.proof
-------------------------------------------------------------------------------
Verification complete in 5 ms
```

## Conclusion

This tutorial demonstrated how to write a basic program using the Miden compiler and how to prove and verify the execution of the program using the Miden CLI.
