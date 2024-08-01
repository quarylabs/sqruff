import subprocess
import csv
from datetime import datetime
import argparse
import re
import sys

def run_command(command):
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=True)
    output, error = process.communicate()
    return output.decode('utf-8'), error.decode('utf-8')

def get_commits(start_date=None, end_date=None):
    command = 'git log --format="%H|%ct|%s"'
    if start_date:
        command += f' --since="{start_date}"'
    if end_date:
        command += f' --until="{end_date}"'
    output, error = run_command(command)

    if error:
        print(f"Error running git log: {error}")
        sys.exit(1)

    if not output.strip():
        print(f"No commits found in the specified date range.")
        sys.exit(1)

    commits = []
    for line in output.strip().split('\n'):
        parts = line.split('|')
        if len(parts) != 3:
            print(f"Unexpected git log output format: {line}")
            continue
        hash, timestamp, message = parts
        commits.append((hash, int(timestamp), message))

    if not commits:
        print(f"No valid commits found in the specified date range.")
        sys.exit(1)

    return commits

def run_benchmark(commit_hash):
    checkout_output, checkout_error = run_command(f"git checkout {commit_hash}")
    if checkout_error:
        print(f"Error checking out commit {commit_hash}: {checkout_error}")
        return None, None, None

    bench_output, bench_error = run_command("cargo bench --bench fix")
    print(bench_output)
    print(bench_error)
    if bench_error:
        print(f"Error running benchmark for commit {commit_hash}: {bench_error}")
        return None, None, None

    return parse_benchmark_output(bench_output + bench_error)

def parse_benchmark_output(output):
    time_match = re.search(r"time:\s+\[(\d+(?:\.\d+)?)\s+(\w+)\s+(\d+(?:\.\d+)?)\s+(\w+)\s+(\d+(?:\.\d+)?)\s+(\w+)\]", output)
    if time_match:
        time = float(time_match.group(1))
        unit = time_match.group(2)
        lower_bound = float(time_match.group(3))
        upper_bound = float(time_match.group(5))
        uncertainty = (upper_bound - lower_bound) / 2
        return time, uncertainty, unit
    else:
        return None, None, None


def main(start_date, end_date, output_file):
    commits = get_commits(start_date, end_date)

    with open(output_file, 'w', newline='') as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(['Commit Hash', 'Commit Time', 'Commit Message', 'Benchmark Time', 'Uncertainty', 'Unit'])

        for commit_hash, timestamp, message in commits:
            print(f"Processing commit: {commit_hash}")
            commit_time = datetime.fromtimestamp(timestamp).isoformat()
            time, uncertainty, unit = run_benchmark(commit_hash)
            if time is not None:
                writer.writerow([commit_hash, commit_time, message, time, uncertainty, unit])
            else:
                print(f"Skipping commit {commit_hash} due to benchmark failure")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Run benchmarks on Git commits within an optional date range.')
    parser.add_argument('--start_date', help='Start date for commit range (YYYY-MM-DD)', default=None)
    parser.add_argument('--end_date', help='End date for commit range (YYYY-MM-DD)', default=None)
    parser.add_argument('output_file', help='Output CSV file name')
    args = parser.parse_args()

    main(args.start_date, args.end_date, args.output_file)