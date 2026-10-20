// assemble_rule_tests fills the unconverted entries of a tools/extract_rule_tests case file with the cases that
// TSRSLINT_DUMP_CASES wrote at runtime. Converted cases keep the extractor's text, so only unconverted lists or
// cases change. Groups match by source order (test file, then line of the RunRuleTester call).
//
// Usage, from tools/assemble_rule_tests: GOWORK=off go run . <dump dir> <rule name> <rule dir> <extracted.json> <out.json>
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
)

type unconverted struct {
	Group  int    `json:"group"`
	Kind   string `json:"kind"`
	Index  int    `json:"index"`
	Line   int    `json:"line"`
	Reason string `json:"reason"`
}

type group struct {
	TSConfig string            `json:"tsconfig"`
	Valid    []json.RawMessage `json:"valid"`
	Invalid  []json.RawMessage `json:"invalid"`
}

type output struct {
	Source      string        `json:"source"`
	Groups      []group       `json:"groups"`
	Unconverted []unconverted `json:"unconverted"`
}

type dumped struct {
	File     string            `json:"file"`
	Line     int               `json:"line"`
	Test     string            `json:"test"`
	TSConfig string            `json:"tsconfig"`
	Valid    []json.RawMessage `json:"valid"`
	Invalid  []json.RawMessage `json:"invalid"`
}

func main() {
	dumpDir, ruleName, ruleDir, extractedPath, outPath := os.Args[1], os.Args[2], os.Args[3], os.Args[4], os.Args[5]
	tests, _ := filepath.Glob(filepath.Join(ruleDir, "*_test.go"))
	order := map[string]int{}
	for i, p := range tests {
		order[filepath.Base(p)] = i
	}
	paths, _ := filepath.Glob(filepath.Join(dumpDir, ruleName+"__*.json"))
	var dumps []dumped
	for _, p := range paths {
		b, err := os.ReadFile(p)
		if err != nil {
			panic(err)
		}
		var d dumped
		if err := json.Unmarshal(b, &d); err != nil {
			panic(err)
		}
		dumps = append(dumps, d)
	}
	sort.Slice(dumps, func(i, j int) bool {
		if dumps[i].File != dumps[j].File {
			return order[dumps[i].File] < order[dumps[j].File]
		}
		if dumps[i].Line != dumps[j].Line {
			return dumps[i].Line < dumps[j].Line
		}
		return dumps[i].Test < dumps[j].Test
	})
	// A call site inside a loop dumps once per iteration: one extracted group holds all of them.
	var merged []dumped
	for _, d := range dumps {
		if n := len(merged); n > 0 && merged[n-1].File == d.File && merged[n-1].Line == d.Line {
			if merged[n-1].TSConfig != d.TSConfig {
				panic(fmt.Sprintf("%s %s:%d: tsconfig differs between iterations", ruleName, d.File, d.Line))
			}
			merged[n-1].Valid = append(merged[n-1].Valid, d.Valid...)
			merged[n-1].Invalid = append(merged[n-1].Invalid, d.Invalid...)
			continue
		}
		merged = append(merged, d)
	}
	dumps = merged

	b, err := os.ReadFile(extractedPath)
	if err != nil {
		panic(err)
	}
	var out output
	if err := json.Unmarshal(b, &out); err != nil {
		panic(err)
	}
	if len(dumps) != len(out.Groups) {
		panic(fmt.Sprintf("%s: %d runtime groups, %d extracted groups", ruleName, len(dumps), len(out.Groups)))
	}
	for gi := range out.Groups {
		if out.Groups[gi].TSConfig != dumps[gi].TSConfig {
			panic(fmt.Sprintf("%s group %d: tsconfig %q != %q", ruleName, gi, out.Groups[gi].TSConfig, dumps[gi].TSConfig))
		}
	}
	for _, u := range out.Unconverted {
		if u.Kind == "tsconfig" {
			panic(fmt.Sprintf("%s group %d: unconverted tsconfig", ruleName, u.Group))
		}
		g, d := &out.Groups[u.Group], dumps[u.Group]
		target, source := &g.Valid, d.Valid
		if u.Kind == "invalid" {
			target, source = &g.Invalid, d.Invalid
		}
		// A whole list (index -1) or a case the extractor skipped: the runtime list holds every case in order.
		*target = source
	}
	out.Unconverted = []unconverted{}
	for gi := range out.Groups {
		if out.Groups[gi].Valid == nil {
			out.Groups[gi].Valid = []json.RawMessage{}
		}
		if out.Groups[gi].Invalid == nil {
			out.Groups[gi].Invalid = []json.RawMessage{}
		}
	}
	res, _ := json.MarshalIndent(out, "", "  ")
	if err := os.WriteFile(outPath, res, 0o644); err != nil {
		panic(err)
	}
}
