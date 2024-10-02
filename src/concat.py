import glob
import os


def concatenate_rust_files():
    current_dir = os.path.dirname(os.path.abspath(__file__))
    rust_files = glob.glob(os.path.join(current_dir, "*.rs"))
    output_file = os.path.join(current_dir, "concat.txt")

    with open(output_file, "w") as outfile:
        for rust_file in rust_files:
            outfile.write(f"# {os.path.basename(rust_file)}\n\n")

            with open(rust_file, "r") as infile:
                outfile.write(infile.read())

            outfile.write("\n\n")

    print(f"All Rust files have been concatenated into '{output_file}'")


if __name__ == "__main__":
    concatenate_rust_files()
