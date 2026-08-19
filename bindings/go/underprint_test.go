package underprint

import (
	"encoding/json"
	"testing"
)

func TestCapabilitiesAreCopiedBeforeNativeResultFree(t *testing.T) {
	context, err := New("", nil)
	if err != nil {
		t.Fatal(err)
	}
	document, err := context.Capabilities()
	if err != nil {
		t.Fatal(err)
	}
	context.Close()
	context.Close()
	var capabilities struct {
		Schema string `json:"schema"`
		Build  struct {
			ABI int `json:"abi_version"`
		} `json:"build"`
	}
	if err := json.Unmarshal(document, &capabilities); err != nil {
		t.Fatal(err)
	}
	if capabilities.Schema != "underprint.capabilities/v1" || capabilities.Build.ABI != ABIVersion {
		t.Fatalf("unexpected capabilities: %s", document)
	}
}

func TestVersionViewIsCopied(t *testing.T) {
	if Version() == "" {
		t.Fatal("expected a native version")
	}
}
