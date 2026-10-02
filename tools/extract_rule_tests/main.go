// extract_rule_tests converts tsgolint's Go rule tests (internal/rules/<rule>/<rule>_test.go) into JSON test
// cases for the Rust harness (crates/tsrslint/tests/rule_tests.rs). It evaluates the composite literals passed
// to rule_tester.RunRuleTester statically with go/ast; cases it cannot evaluate are listed in "unconverted".
//
// Usage: GOWORK=off go run ./tools/extract_rule_tests <rule dir> <out.json>
package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
)

type suggestion struct {
	MessageId string `json:"messageId"`
	Output    string `json:"output"`
}

type caseError struct {
	MessageId   string       `json:"messageId"`
	Line        int          `json:"line,omitempty"`
	Column      int          `json:"column,omitempty"`
	EndLine     int          `json:"endLine,omitempty"`
	EndColumn   int          `json:"endColumn,omitempty"`
	Suggestions []suggestion `json:"suggestions"`
}

type testCase struct {
	Code     string            `json:"code"`
	Only     bool              `json:"only,omitempty"`
	Skip     bool              `json:"skip,omitempty"`
	FileName string            `json:"fileName,omitempty"`
	Options  any               `json:"options,omitempty"`
	TSConfig string            `json:"tsconfig,omitempty"`
	Tsx      bool              `json:"tsx,omitempty"`
	Files    map[string]string `json:"files,omitempty"`
	Output   []string          `json:"output,omitempty"`
	Errors   []caseError       `json:"errors,omitempty"`
}

type group struct {
	TSConfig string     `json:"tsconfig"`
	Valid    []testCase `json:"valid"`
	Invalid  []testCase `json:"invalid"`
}

type unconverted struct {
	Group  int    `json:"group"`
	Kind   string `json:"kind"`
	Index  int    `json:"index"`
	Line   int    `json:"line"`
	Reason string `json:"reason"`
}

type output struct {
	Source      string        `json:"source"`
	Groups      []group       `json:"groups"`
	Unconverted []unconverted `json:"unconverted"`
}

type evaluator struct {
	fset   *token.FileSet
	consts map[string]any
	// struct type name -> Go field name -> JSON name
	jsonNames map[string]map[string]string
	locals    map[string]ast.Expr
}

type evalError struct{ msg string }

func (e evalError) Error() string { return e.msg }

func fail(format string, args ...any) { panic(evalError{fmt.Sprintf(format, args...)}) }

func lowerFirst(s string) string {
	if s == "" {
		return s
	}
	return strings.ToLower(s[:1]) + s[1:]
}

func (ev *evaluator) str(e ast.Expr) string {
	v := ev.value(e, "")
	s, ok := v.(string)
	if !ok {
		fail("expected string, got %T", v)
	}
	return s
}

func (ev *evaluator) value(e ast.Expr, typeHint string) any {
	switch x := e.(type) {
	case *ast.BasicLit:
		switch x.Kind {
		case token.STRING:
			s, err := strconv.Unquote(x.Value)
			if err != nil {
				fail("bad string: %v", err)
			}
			return s
		case token.INT:
			n, _ := strconv.Atoi(x.Value)
			return n
		case token.FLOAT:
			f, _ := strconv.ParseFloat(x.Value, 64)
			return f
		}
	case *ast.BinaryExpr:
		if x.Op == token.ADD {
			return ev.str(x.X) + ev.str(x.Y)
		}
	case *ast.ParenExpr:
		return ev.value(x.X, typeHint)
	case *ast.UnaryExpr:
		if x.Op == token.AND {
			return ev.value(x.X, typeHint)
		}
		if x.Op == token.SUB {
			v := ev.value(x.X, typeHint)
			if n, ok := v.(int); ok {
				return -n
			}
		}
	case *ast.Ident:
		switch x.Name {
		case "true":
			return true
		case "false":
			return false
		case "nil":
			return nil
		}
		if v, ok := ev.consts[x.Name]; ok {
			return v
		}
		if l, ok := ev.locals[x.Name]; ok {
			return ev.value(l, typeHint)
		}
		fail("unknown identifier %s", x.Name)
	case *ast.SelectorExpr:
		if v, ok := ev.consts[x.Sel.Name]; ok {
			return v
		}
		fail("unknown selector %s", x.Sel.Name)
	case *ast.CallExpr:
		fun := x.Fun
		if ix, ok := fun.(*ast.IndexExpr); ok {
			fun = ix.X
		}
		if ix, ok := fun.(*ast.IndexListExpr); ok {
			fun = ix.X
		}
		name := ""
		if sel, ok := fun.(*ast.SelectorExpr); ok {
			name = sel.Sel.Name
		} else if id, ok := fun.(*ast.Ident); ok {
			name = id.Name
		}
		switch name {
		case "OptionsFromJSON":
			var v any
			if err := json.Unmarshal([]byte(ev.str(x.Args[0])), &v); err != nil {
				fail("bad options JSON: %v", err)
			}
			return v
		case "Ref":
			return ev.value(x.Args[0], typeHint)
		case "Dedent", "dedent":
			fail("helper %s not supported", name)
		}
		// Type conversion such as ReturnAwaitOptions("always").
		if len(x.Args) == 1 {
			if _, ok := fun.(*ast.Ident); ok {
				return ev.value(x.Args[0], typeHint)
			}
		}
		fail("call to %s not supported", name)
	case *ast.CompositeLit:
		return ev.composite(x, typeHint)
	}
	fail("unsupported expression %T at %s", e, ev.fset.Position(e.Pos()))
	return nil
}

func typeName(t ast.Expr) string {
	switch x := t.(type) {
	case *ast.Ident:
		return x.Name
	case *ast.SelectorExpr:
		return x.Sel.Name
	case *ast.StarExpr:
		return typeName(x.X)
	}
	return ""
}

func (ev *evaluator) composite(x *ast.CompositeLit, typeHint string) any {
	switch t := x.Type.(type) {
	case *ast.ArrayType:
		elem := typeName(t.Elt)
		out := make([]any, 0, len(x.Elts))
		for _, e := range x.Elts {
			out = append(out, ev.value(e, elem))
		}
		return out
	case *ast.MapType:
		out := map[string]any{}
		for _, e := range x.Elts {
			kv := e.(*ast.KeyValueExpr)
			out[ev.str(kv.Key)] = ev.value(kv.Value, typeName(t.Value))
		}
		return out
	}
	name := typeName(x.Type)
	if name == "" {
		name = typeHint
	}
	out := map[string]any{}
	names := ev.jsonNames[name]
	for _, e := range x.Elts {
		kv, ok := e.(*ast.KeyValueExpr)
		if !ok {
			fail("positional struct literal for %s", name)
		}
		field := kv.Key.(*ast.Ident).Name
		jsonName := lowerFirst(field)
		if names != nil {
			if n, ok := names[field]; ok {
				jsonName = n
			}
		}
		out[jsonName] = ev.value(kv.Value, "")
	}
	return out
}

func toCase(m map[string]any) testCase {
	var c testCase
	for k, v := range m {
		switch k {
		case "code":
			c.Code = v.(string)
		case "only":
			c.Only = v.(bool)
		case "skip":
			c.Skip = v.(bool)
		case "fileName":
			c.FileName = v.(string)
		case "options":
			c.Options = v
		case "tSConfig", "tsconfig":
			c.TSConfig = v.(string)
		case "tsx":
			c.Tsx = v.(bool)
		case "files":
			c.Files = map[string]string{}
			for fk, fv := range v.(map[string]any) {
				c.Files[fk] = fv.(string)
			}
		case "output":
			for _, o := range v.([]any) {
				c.Output = append(c.Output, o.(string))
			}
		case "errors":
			for _, ei := range v.([]any) {
				em := ei.(map[string]any)
				var ce caseError
				ce.Suggestions = []suggestion{}
				for ek, evv := range em {
					switch ek {
					case "messageId":
						ce.MessageId = evv.(string)
					case "line":
						ce.Line = evv.(int)
					case "column":
						ce.Column = evv.(int)
					case "endLine":
						ce.EndLine = evv.(int)
					case "endColumn":
						ce.EndColumn = evv.(int)
					case "suggestions":
						for _, si := range evv.([]any) {
							sm := si.(map[string]any)
							s := suggestion{}
							if id, ok := sm["messageId"].(string); ok {
								s.MessageId = id
							}
							if o, ok := sm["output"].(string); ok {
								s.Output = o
							}
							ce.Suggestions = append(ce.Suggestions, s)
						}
					default:
						fail("unknown error field %s", ek)
					}
				}
				c.Errors = append(c.Errors, ce)
			}
		default:
			fail("unknown test case field %s", k)
		}
	}
	return c
}

func main() {
	ruleDir, outPath := os.Args[1], os.Args[2]
	fset := token.NewFileSet()
	ev := &evaluator{fset: fset, consts: map[string]any{}, jsonNames: map[string]map[string]string{}}
	matches, _ := filepath.Glob(filepath.Join(ruleDir, "*.go"))
	var testFiles []*ast.File
	var testPaths []string
	for _, p := range matches {
		f, err := parser.ParseFile(fset, p, nil, parser.ParseComments)
		if err != nil {
			panic(err)
		}
		if strings.HasSuffix(p, "_test.go") {
			testFiles = append(testFiles, f)
			testPaths = append(testPaths, filepath.Base(p))
		}
		for _, d := range f.Decls {
			gd, ok := d.(*ast.GenDecl)
			if !ok {
				continue
			}
			for _, s := range gd.Specs {
				switch sp := s.(type) {
				case *ast.ValueSpec:
					if gd.Tok == token.CONST {
						for i, n := range sp.Names {
							if i < len(sp.Values) {
								if bl, ok := sp.Values[i].(*ast.BasicLit); ok && bl.Kind == token.STRING {
									v, _ := strconv.Unquote(bl.Value)
									ev.consts[n.Name] = v
								}
							}
						}
					}
				case *ast.TypeSpec:
					st, ok := sp.Type.(*ast.StructType)
					if !ok {
						continue
					}
					m := map[string]string{}
					for _, fld := range st.Fields.List {
						if fld.Tag == nil || len(fld.Names) == 0 {
							continue
						}
						tag, _ := strconv.Unquote(fld.Tag.Value)
						j := reflect.StructTag(tag).Get("json")
						if j != "" {
							m[fld.Names[0].Name] = strings.Split(j, ",")[0]
						}
					}
					ev.jsonNames[sp.Name.Name] = m
				}
			}
		}
	}
	out := output{Source: strings.Join(testPaths, ","), Groups: []group{}, Unconverted: []unconverted{}}
	for _, f := range testFiles {
		ast.Inspect(f, func(n ast.Node) bool {
			fd, ok := n.(*ast.FuncDecl)
			if !ok || fd.Body == nil {
				return true
			}
			ev.locals = map[string]ast.Expr{}
			ast.Inspect(fd.Body, func(n ast.Node) bool {
				if as, ok := n.(*ast.AssignStmt); ok && len(as.Lhs) == len(as.Rhs) {
					for i, l := range as.Lhs {
						if id, ok := l.(*ast.Ident); ok {
							ev.locals[id.Name] = as.Rhs[i]
						}
					}
				}
				call, ok := n.(*ast.CallExpr)
				if !ok {
					return true
				}
				sel, ok := call.Fun.(*ast.SelectorExpr)
				if !ok || sel.Sel.Name != "RunRuleTester" || len(call.Args) != 6 {
					return true
				}
				gi := len(out.Groups)
				g := group{Valid: []testCase{}, Invalid: []testCase{}}
				func() {
					defer func() {
						if r := recover(); r != nil {
							out.Unconverted = append(out.Unconverted, unconverted{Group: gi, Kind: "tsconfig", Reason: fmt.Sprint(r)})
						}
					}()
					g.TSConfig = ev.str(call.Args[1])
				}()
				for ai, kind := range []string{"valid", "invalid"} {
					arg := call.Args[4+ai]
					if id, ok := arg.(*ast.Ident); ok {
						arg = ev.locals[id.Name]
					}
					lit, ok := arg.(*ast.CompositeLit)
					if !ok {
						out.Unconverted = append(out.Unconverted, unconverted{Group: gi, Kind: kind, Index: -1, Line: fset.Position(call.Pos()).Line, Reason: fmt.Sprintf("%s cases are not a literal (%T)", kind, arg)})
						continue
					}
					for idx, e := range lit.Elts {
						func() {
							defer func() {
								if r := recover(); r != nil {
									if ee, ok := r.(evalError); ok {
										out.Unconverted = append(out.Unconverted, unconverted{Group: gi, Kind: kind, Index: idx, Line: fset.Position(e.Pos()).Line, Reason: ee.msg})
										return
									}
									out.Unconverted = append(out.Unconverted, unconverted{Group: gi, Kind: kind, Index: idx, Line: fset.Position(e.Pos()).Line, Reason: fmt.Sprint(r)})
								}
							}()
							m := ev.value(e, "TestCase").(map[string]any)
							c := toCase(m)
							if kind == "valid" {
								g.Valid = append(g.Valid, c)
							} else {
								g.Invalid = append(g.Invalid, c)
							}
						}()
					}
				}
				out.Groups = append(out.Groups, g)
				return true
			})
			return false
		})
	}
	b, _ := json.MarshalIndent(out, "", "  ")
	if err := os.WriteFile(outPath, b, 0o644); err != nil {
		panic(err)
	}
	nv, ni := 0, 0
	for _, g := range out.Groups {
		nv += len(g.Valid)
		ni += len(g.Invalid)
	}
	fmt.Printf("%s: %d groups, %d valid, %d invalid, %d unconverted\n", filepath.Base(ruleDir), len(out.Groups), nv, ni, len(out.Unconverted))
}
