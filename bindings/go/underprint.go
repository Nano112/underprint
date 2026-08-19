package underprint

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo darwin LDFLAGS: -L${SRCDIR}/../../target/minimal-release -lunderprint
#cgo linux LDFLAGS: -L${SRCDIR}/../../target/minimal-release -lunderprint
#include <stdlib.h>
#include "underprint.h"
*/
import "C"

import (
	"encoding/json"
	"errors"
	"runtime"
	"sync"
	"unsafe"
)

const ABIVersion = 1

type Error struct {
	Status   int
	Code     string `json:"code"`
	Message  string `json:"message"`
	Document json.RawMessage
}

func (e *Error) Error() string {
	if e.Message != "" {
		return e.Message
	}
	return "underprint operation failed"
}

type Context struct {
	mu     sync.RWMutex
	handle *C.up_context
}

type Embedding struct {
	Image  []byte
	Report json.RawMessage
}

func New(modelsDirectory string, runtimeConfiguration map[string]any) (*Context, error) {
	if uint32(C.up_abi_version()) != ABIVersion {
		return nil, errors.New("underprint ABI version mismatch")
	}
	configuration := make(map[string]any, len(runtimeConfiguration)+1)
	for key, value := range runtimeConfiguration {
		configuration[key] = value
	}
	if modelsDirectory != "" {
		configuration["models_dir"] = modelsDirectory
	}
	encoded, err := json.Marshal(configuration)
	if err != nil {
		return nil, err
	}
	view, release := bytesView(encoded)
	defer release()
	var handle *C.up_context
	status := int(C.up_context_create(view, &handle))
	if status != 0 {
		return nil, &Error{Status: status, Message: "underprint context initialization failed"}
	}
	context := &Context{handle: handle}
	runtime.SetFinalizer(context, (*Context).Close)
	return context, nil
}

func Version() string {
	return copyView(C.up_version())
}

func (context *Context) Capabilities() (json.RawMessage, error) {
	return context.call(func(handle *C.up_context, result **C.up_result) int {
		return int(C.up_context_capabilities(handle, result))
	}, false)
}

func (context *Context) Detect(image []byte, profile string) (json.RawMessage, bool, error) {
	options, err := json.Marshal(map[string]string{"profile": profile})
	if err != nil {
		return nil, false, err
	}
	imageView, releaseImage := bytesView(image)
	defer releaseImage()
	optionsView, releaseOptions := bytesView(options)
	defer releaseOptions()
	document, status, err := context.callStatus(func(handle *C.up_context, result **C.up_result) int {
		return int(C.up_detect(handle, imageView, optionsView, result))
	}, true)
	return document, status == 0, err
}

func (context *Context) Embed(image []byte, payload, profile string) (*Embedding, error) {
	options, err := json.Marshal(map[string]string{"payload": payload, "profile": profile})
	if err != nil {
		return nil, err
	}
	imageView, releaseImage := bytesView(image)
	defer releaseImage()
	optionsView, releaseOptions := bytesView(options)
	defer releaseOptions()

	context.mu.RLock()
	defer context.mu.RUnlock()
	if context.handle == nil {
		return nil, errors.New("underprint context is closed")
	}
	var result *C.up_result
	status := int(C.up_embed(context.handle, imageView, optionsView, &result))
	if result == nil {
		return nil, &Error{Status: status, Message: "underprint returned no result"}
	}
	defer C.up_result_free(result)
	document := json.RawMessage(copyView(C.up_result_json(result)))
	if status != 0 {
		return nil, decodeError(status, document)
	}
	return &Embedding{
		Image:  []byte(copyView(C.up_result_output(result))),
		Report: append(json.RawMessage(nil), document...),
	}, nil
}

func (context *Context) Close() {
	context.mu.Lock()
	defer context.mu.Unlock()
	if context.handle != nil {
		C.up_context_free(context.handle)
		context.handle = nil
	}
	runtime.SetFinalizer(context, nil)
}

func (context *Context) call(operation func(*C.up_context, **C.up_result) int, allowNotDetected bool) (json.RawMessage, error) {
	document, _, err := context.callStatus(operation, allowNotDetected)
	return document, err
}

func (context *Context) callStatus(operation func(*C.up_context, **C.up_result) int, allowNotDetected bool) (json.RawMessage, int, error) {
	context.mu.RLock()
	defer context.mu.RUnlock()
	if context.handle == nil {
		return nil, 2, errors.New("underprint context is closed")
	}
	var result *C.up_result
	status := operation(context.handle, &result)
	if result == nil {
		return nil, status, &Error{Status: status, Message: "underprint returned no result"}
	}
	defer C.up_result_free(result)
	document := json.RawMessage(copyView(C.up_result_json(result)))
	if status != 0 && !(allowNotDetected && status == 1) {
		return nil, status, decodeError(status, document)
	}
	return append(json.RawMessage(nil), document...), status, nil
}

func decodeError(status int, document json.RawMessage) error {
	native := &Error{Status: status, Document: append(json.RawMessage(nil), document...)}
	_ = json.Unmarshal(document, native)
	return native
}

func bytesView(bytes []byte) (C.up_bytes_view, func()) {
	if len(bytes) == 0 {
		return C.up_bytes_view{}, func() {}
	}
	pointer := C.CBytes(bytes)
	return C.up_bytes_view{data: (*C.uint8_t)(pointer), len: C.size_t(len(bytes))}, func() {
		C.free(pointer)
	}
}

func copyView(view C.up_bytes_view) string {
	if view.data == nil || view.len == 0 {
		return ""
	}
	return string(C.GoBytes(unsafe.Pointer(view.data), C.int(view.len)))
}
