#include "Driver.h"
#include "Driver.tmh"

using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

IndirectMonitorContext::IndirectMonitorContext(_In_ IDDCX_MONITOR Monitor, UINT ConnectorIndex) :
  m_Monitor(Monitor) {
  m_RustMonitor.reset(new MonitorClient(ConnectorIndex));
}

IndirectMonitorContext::~IndirectMonitorContext() {
  m_ProcessingThread.reset();
  m_RustMonitor.reset();
}

void IndirectMonitorContext::Configure(DISPLAYCONFIG_VIDEO_SIGNAL_INFO& Mode) {
  auto width = Mode.totalSize.cx;
  auto height = Mode.totalSize.cy;
  auto framerate = Mode.vSyncFreq;

  m_RustMonitor->Configure(width, height, framerate.Numerator / framerate.Denominator);
}

void IndirectMonitorContext::AssignSwapChain(IDDCX_SWAPCHAIN SwapChain, LUID RenderAdapter, HANDLE NewFrameEvent) {
  m_ProcessingThread.reset();

  auto Device = std::make_shared<Direct3DDevice>(RenderAdapter);
  if (FAILED(Device->Init())) {
    // It's important to delete the swap-chain if D3D initialization fails, so that the OS knows to generate a new
    // swap-chain and try again.
    WdfObjectDelete(SwapChain);
  }
  else {
    // Create a new swap-chain processing thread
    m_ProcessingThread.reset(new SwapChainProcessor(SwapChain, Device, NewFrameEvent, m_RustMonitor));
  }
}

void IndirectMonitorContext::UnassignSwapChain() {
  // Stop processing the last swap-chain
  m_ProcessingThread.reset();
}