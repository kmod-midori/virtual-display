#include "Driver.h"
#include "Driver.tmh"

using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

DWORD CALLBACK RunCursorThread(LPVOID Argument) {
  reinterpret_cast<IndirectMonitorContext*>(Argument)->CursorThread();
  return 0;
}

IndirectMonitorContext::IndirectMonitorContext(_In_ IDDCX_MONITOR Monitor, UINT ConnectorIndex) :
  m_Monitor(Monitor) {
  m_RustMonitor.reset(new MonitorClient(ConnectorIndex));
  m_hNewCursorDataAvailableEvent.Attach(CreateEvent(nullptr, false, false, nullptr));
  m_hTerminateCursorThreadEvent.Attach(CreateEvent(nullptr, false, false, nullptr));
  m_CursorBuffer = (PBYTE)malloc(CURSOR_BUFFER_SIZE);
  m_hCursorThread.Attach(CreateThread(nullptr, 0, RunCursorThread, this, 0, nullptr));
}

IndirectMonitorContext::~IndirectMonitorContext() {
  SetEvent(m_hTerminateCursorThreadEvent.Get());

  if (m_hCursorThread.Get()) {
    WaitForSingleObject(m_hCursorThread.Get(), INFINITE);
  }

  m_SwapChainProcessor.reset();
  m_RustMonitor.reset();
  free(m_CursorBuffer);
}

void IndirectMonitorContext::CommitModes(DISPLAYCONFIG_VIDEO_SIGNAL_INFO& Mode) {
  auto width = Mode.totalSize.cx;
  auto height = Mode.totalSize.cy;
  auto framerate = Mode.vSyncFreq;

  m_RustMonitor->CommitModes(width, height, framerate.Numerator / framerate.Denominator);

  IDARG_IN_SETUP_HWCURSOR cursorSetup = {};
  cursorSetup.CursorInfo.Size = sizeof(IDDCX_CURSOR_CAPS);
  // Prefer software rendering for XOR cursors
  cursorSetup.CursorInfo.ColorXorCursorSupport = IDDCX_XOR_CURSOR_SUPPORT_NONE;
  // Alpha is supported
  cursorSetup.CursorInfo.AlphaCursorSupport = true;
  // Maximum cursor size
  cursorSetup.CursorInfo.MaxX = CURSOR_MAX_WIDTH;
  cursorSetup.CursorInfo.MaxY = CURSOR_MAX_HEIGHT;

  cursorSetup.hNewCursorDataAvailable = m_hNewCursorDataAvailableEvent.Get();

  if (!NT_SUCCESS(IddCxMonitorSetupHardwareCursor(m_Monitor, &cursorSetup))) {
    return;
  }
}

void IndirectMonitorContext::AssignSwapChain(IDDCX_SWAPCHAIN SwapChain, LUID RenderAdapter, HANDLE NewFrameEvent) {
  m_SwapChainProcessor.reset();

  auto Device = std::make_shared<Direct3DDevice>(RenderAdapter);
  if (FAILED(Device->Init())) {
    // It's important to delete the swap-chain if D3D initialization fails, so that the OS knows to generate a new
    // swap-chain and try again.
    WdfObjectDelete(SwapChain);
  }
  else {
    // Create a new swap-chain processing thread
    m_SwapChainProcessor.reset(new SwapChainProcessor(SwapChain, Device, NewFrameEvent, m_RustMonitor));
  }
}

void IndirectMonitorContext::UnassignSwapChain() {
  // Stop processing the last swap-chain
  m_SwapChainProcessor.reset();
}

void IndirectMonitorContext::CursorThread() {
  for (;;) {
    HANDLE WaitHandles[] =
    {
        m_hNewCursorDataAvailableEvent.Get(),
        m_hTerminateCursorThreadEvent.Get()
    };

    DWORD WaitResult = WaitForMultipleObjects(ARRAYSIZE(WaitHandles), WaitHandles, FALSE, INFINITE);

    if (WaitResult == WAIT_OBJECT_0) {
      // Updated
      IDARG_IN_QUERY_HWCURSOR QueryArgs = {};
      QueryArgs.LastShapeId = m_CursorShapeId;
      QueryArgs.ShapeBufferSizeInBytes = CURSOR_BUFFER_SIZE;
      QueryArgs.pShapeBuffer = m_CursorBuffer;
      IDARG_OUT_QUERY_HWCURSOR QueryOutput = {};
      NTSTATUS Status = IddCxMonitorQueryHardwareCursor(m_Monitor, &QueryArgs, &QueryOutput);
      if (!NT_SUCCESS(Status)) {
        // It sometimes fails, just try again
        continue;
      }
      if (QueryOutput.IsCursorShapeUpdated && QueryOutput.IsCursorVisible) {
        m_RustMonitor->UpdateCursorImage(
          QueryOutput.CursorShapeInfo.Width,
          QueryOutput.CursorShapeInfo.Height,
          m_CursorBuffer,
          QueryOutput.CursorShapeInfo.Pitch
        );
        m_CursorShapeId = QueryOutput.CursorShapeInfo.ShapeId;
      }
      m_RustMonitor->UpdateCursorPosition(QueryOutput.X, QueryOutput.Y, QueryOutput.IsCursorVisible);
    }
    else if (WaitResult == WAIT_OBJECT_0 + 1) {
      // Terminated
      break;
    }
    else {
      // Failed
      break;
    }
  }
}