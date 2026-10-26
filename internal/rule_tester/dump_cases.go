package rule_tester

// Writes the cases of each RunRuleTester call as JSON in the schema of tools/extract_rule_tests, for test cases
// that the static extractor cannot evaluate. Enabled by TSRSLINT_DUMP_CASES=<dir>.

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	jsonv2 "github.com/go-json-experiment/json"
)

type dumpSuggestion struct {
	MessageId string `json:"messageId"`
	Output    string `json:"output"`
}

type dumpError struct {
	MessageId   string           `json:"messageId"`
	Line        int              `json:"line,omitempty"`
	Column      int              `json:"column,omitempty"`
	EndLine     int              `json:"endLine,omitempty"`
	EndColumn   int              `json:"endColumn,omitempty"`
	Suggestions []dumpSuggestion `json:"suggestions"`
}

type dumpCase struct {
	Code     string            `json:"code"`
	Only     bool              `json:"only,omitempty"`
	Skip     bool              `json:"skip,omitempty"`
	FileName string            `json:"fileName,omitempty"`
	Options  json.RawMessage   `json:"options,omitempty"`
	TSConfig string            `json:"tsconfig,omitempty"`
	Tsx      bool              `json:"tsx,omitempty"`
	Files    map[string]string `json:"files,omitempty"`
	Output   []string          `json:"output,omitempty"`
	Errors   []dumpError       `json:"errors,omitempty"`
}

type dumpGroup struct {
	File     string     `json:"file"`
	Line     int        `json:"line"`
	Test     string     `json:"test"`
	TSConfig string     `json:"tsconfig"`
	Valid    []dumpCase `json:"valid"`
	Invalid  []dumpCase `json:"invalid"`
}

// The rule receives the options struct itself, so a false field is an explicit false. encoding/json drops it under
// `omitempty`, and the Rust harness would then apply the JSON default; jsonv2 keeps it.
func dumpOptions(options any) json.RawMessage {
	if options == nil {
		return nil
	}
	b, err := jsonv2.Marshal(options)
	if err != nil {
		panic(err)
	}
	return b
}

// A call site inside a loop runs once per iteration, so the file name includes the test name.
func dumpCases(dir string, t *testing.T, tsconfigPath string, ruleName string, valid []ValidTestCase, invalid []InvalidTestCase) {
	_, file, line, _ := runtime.Caller(2)
	g := dumpGroup{File: filepath.Base(file), Line: line, Test: t.Name(), TSConfig: tsconfigPath, Valid: []dumpCase{}, Invalid: []dumpCase{}}
	for _, c := range valid {
		g.Valid = append(g.Valid, dumpCase{Code: c.Code, Only: c.Only, Skip: c.Skip, FileName: c.FileName, Options: dumpOptions(c.Options), TSConfig: c.TSConfig, Tsx: c.Tsx, Files: c.Files})
	}
	for _, c := range invalid {
		errs := []dumpError{}
		for _, e := range c.Errors {
			s := []dumpSuggestion{}
			for _, x := range e.Suggestions {
				s = append(s, dumpSuggestion{MessageId: x.MessageId, Output: x.Output})
			}
			errs = append(errs, dumpError{MessageId: e.MessageId, Line: e.Line, Column: e.Column, EndLine: e.EndLine, EndColumn: e.EndColumn, Suggestions: s})
		}
		g.Invalid = append(g.Invalid, dumpCase{Code: c.Code, Only: c.Only, Skip: c.Skip, FileName: c.FileName, Options: dumpOptions(c.Options), TSConfig: c.TSConfig, Tsx: c.Tsx, Files: c.Files, Output: c.Output, Errors: errs})
	}
	data, err := json.Marshal(g)
	if err != nil {
		panic(err)
	}
	testName := strings.NewReplacer("/", "_", " ", "_").Replace(t.Name())
	out := filepath.Join(dir, fmt.Sprintf("%s__%s__%06d__%s.json", ruleName, filepath.Base(file), line, testName))
	if err := os.WriteFile(out, data, 0o644); err != nil {
		panic(err)
	}
}
