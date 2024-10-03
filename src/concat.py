import glob
import os
import sys


def concatenate_files(extension):
    current_dir = os.path.dirname(os.path.abspath(__file__))
    files = glob.glob(os.path.join(current_dir, f"**/*.{extension}"), recursive=True)
    output_file = os.path.join(current_dir, "concat.txt")

    with open(output_file, "w") as outfile:
        for file in files:
            relative_path = os.path.relpath(file, current_dir)
            outfile.write(f"# {relative_path}\n\n")

            with open(file, "r") as infile:
                outfile.write(infile.read())

            outfile.write("\n\n")

    print(f"All {extension} files have been concatenated into '{output_file}'")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        usage_message = "Usage:  python concat.py <file_extension>\n"
        result_message = (
            "Result: Concatenates all files with the specified extension\n"
            "        in the current directory and its subdirectories\n"
            "        into a single file named 'concat.txt'"
        )

        print(usage_message)
        print(result_message)
        sys.exit(1)

    file_extension = sys.argv[1]
    concatenate_files(file_extension)
