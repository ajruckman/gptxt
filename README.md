## Text processing via natural language descriptions

Uses [RustPython](https://github.com/RustPython/RustPython) for compiling GPT-4-generated programs to Rust on-the-fly.

### Usage

```
Usage: gptxt [OPTIONS] <task>

Arguments:
  <task>  Description of a text processing task

Options:
  -t, --temp <temp>              Set GPT randomness/temperature (0.05-1.0; lower = more deterministic) [default: 0.25]
  -m, --max-tokens <max-tokens>  Set GPT response token limit [default: 512]
  -j, --json                     Serialize program output to JSON
      --json-one-line            Serialize JSON output to one line (requires --json)
  -i, --input <input>            Read data from a file instead of STDIN
  -s, --show-lines <show-lines>  Show GPT the first N lines of the input to help it generate the program
  -p, --show-prompt              Print the prompt, including the system message and any included lines
  -h, --help                     Print help
  -V, --version                  Print version
```

### Examples

```bash
ajr@ajr-desktop-h /a/r/c/d/r/gptxt (main)> target/release/gptxt \
  "convert this table to a JSON object keyed by phone number, ignoring empty lines" \
  -i examples/test.psv \
  --show-lines 3 \
  --show-prompt \
  | jq
Prompt:
------------------------------
# You are part of a tool that creates Python code for text processing.
# You should return only Python code with no comments.
# Do not describe the code or add any additional information about the code.
# Data to process is stored in the string variable `data`.
# Results should be stored in the variable `result`.

import sys
data = sys.stdin.read()

# First 3 lines of `data`:
#>Name|Age|Email|Phone|City
#>Maria Rodriguez|27|mrodriguez@gmail.com|(555) 123-4567|Miami
#>John Smith|42|johnsmith@yahoo.com|(555) 987-6543|New York

# convert this table to a JSON object keyed by phone number, ignoring empty lines:
------------------------------

Generated program:
------------------------------
import json

lines = data.split('\n')
result = {}
for line in lines[1:]:
    if line:
        name, age, email, phone, city = line.split('|')
        result[phone] = {
            'name': name,
            'age': age,
            'email': email,
            'city': city
        }

result = json.dumps(result)
------------------------------
Run program? ([y]es/[q]uit/[r]egen/[e]dit) y
```

Output:
```json
{
  "(555) 123-4567": {
    "name": "Maria Rodriguez",
    "age": "27",
    "email": "mrodriguez@gmail.com",
    "city": "Miami"
  },
  "(555) 987-6543": {
    "name": "John Smith",
    "age": "42",
    "email": "johnsmith@yahoo.com",
    "city": "New York"
  },
  "(555) 555-1212": {
    "name": "Alex Lee",
    "age": "35",
    "email": "alexlee@hotmail.com",
    "city": "Los Angeles"
  },
  "(555) 555-5555": {
    "name": "Chris Nguyen",
    "age": "29",
    "email": "chrisnguyen@gmail.com",
    "city": "Houston"
  },
  "(555) 321-4567": {
    "name": "David Kim",
    "age": "23",
    "email": "davidkim@gmail.com",
    "city": "Seattle"
  },
  "(555) 555-1234": {
    "name": "Jessica Brown",
    "age": "38",
    "email": "jessicabrown@hotmail.com",
    "city": "Chicago"
  },
  "(555) 444-4444": {
    "name": "Emily Davis",
    "age": "26",
    "email": "emilydavis@yahoo.com",
    "city": "Atlanta"
  }
}
```

-----

```bash
ajr@ajr-desktop-h /a/r/c/d/r/gptxt (main)> cat examples/test.psv | target/release/gptxt \
  "get values in the 'City' column for rows which aren't empty as a string delimited by ':', skipping the column header" \
  --show-lines 3
Generated program:
------------------------------
result = ':'.join([row.split('|')[4] for row in data.splitlines()[1:] if row])
------------------------------
Run program? ([y]es/[q]uit/[r]egen/[e]dit) y
```

Output:
```
Miami:New York:Los Angeles:San Francisco:Seattle:Chicago:Houston:Atlanta
```