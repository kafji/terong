package main

import (
	"fmt"
	"iter"
	"math/rand/v2"
	"os"
	"path"
	"runtime"
	"slices"
	"sync"
	"sync/atomic"
)

func main() {
	path := os.Args[1]

	tree := buildTree(path)

	count := tree.count.Load()
	fmt.Printf("Found %d files.\n", count)

	n := rand.IntN(int(count))

	i := 0
	for f := range tree.files() {
		if i == n {
			fmt.Println(f.path())
			break
		}
		i++
	}
}

type node struct {
	name     string
	dir      bool
	children []*node
	parent   *node
	tree     *tree
}

func (n *node) addChild(child *node) {
	child.parent = n
	child.tree = n.tree
	if !child.dir {
		n.tree.count.Add(1)
	}
	n.children = append(n.children, child)
}

func (n *node) path() string {
	if n.parent == nil {
		return n.name
	}
	return path.Join(n.parent.path(), n.name)
}

func nodeFromDirEntry(entry os.DirEntry) *node {
	typ := entry.Type()
	if !typ.IsDir() && !typ.IsRegular() {
		return nil
	}

	name := entry.Name()

	if slices.Contains([]string{"$RECYCLE.BIN", "System Volume Information"}, name) {
		return nil
	}

	child := &node{name: name, dir: typ.IsDir()}
	return child
}

type tree struct {
	root  *node
	count atomic.Int32
}

func (t *tree) files() iter.Seq[*node] {
	return func(yield func(v *node) bool) {
		trail := make([]*node, 0)
		trail = append(trail, t.root)

		for len(trail) > 0 {
			last := trail[len(trail)-1]
			trail = trail[:len(trail)-1]

			if !last.dir {
				if !yield(last) {
					break
				}
				continue
			}

			trail = append(trail, last.children...)
		}
	}
}

func buildTree(rootPath string) *tree {
	tree1 := &tree{}

	root := &node{
		name: rootPath,
		dir:  true,
		tree: tree1,
	}
	tree1.root = root

	entries, err := os.ReadDir(rootPath)
	if err != nil {
		panic(err)
	}

	for _, entry := range entries {
		child := nodeFromDirEntry(entry)
		if child != nil {
			root.addChild(child)
		}
	}

	token := make(chan struct{}, runtime.GOMAXPROCS(0))
	latch := new(sync.WaitGroup)
	leaves := root.children
	for len(leaves) > 0 {
		mu := new(sync.Mutex)
		leaves2 := make([]*node, 0)
		for len(leaves) > 0 {
			leaf := leaves[len(leaves)-1]
			leaves = leaves[:len(leaves)-1]

			latch.Add(1)
			go func() {
				defer latch.Done()

				token <- struct{}{}
				defer func() {
					<-token
				}()

				more := expandLeaf(leaf)

				mu.Lock()
				defer mu.Unlock()
				leaves2 = append(leaves2, more...)
			}()
		}
		latch.Wait()
		leaves = leaves2
	}

	return tree1
}

func expandLeaf(leaf *node) []*node {
	if !leaf.dir {
		return nil
	}

	path := leaf.path()

	entries, err := os.ReadDir(path)
	if err != nil {
		panic(err)
	}

	for _, entry := range entries {
		child := nodeFromDirEntry(entry)
		if child != nil {
			leaf.addChild(child)
		}
	}

	return leaf.children
}
