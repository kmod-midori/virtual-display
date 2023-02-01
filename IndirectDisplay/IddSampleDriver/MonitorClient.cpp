#include "Driver.h"
#include "Driver.tmh"

#include <string>

using namespace std;
using namespace Microsoft::IndirectDisp;
using namespace Microsoft::WRL;

#define MAX_FB_SIZE (1024 * 1024 * 20)

struct MonitorConfiguration {
  uint32_t configured;
  uint32_t width;
  uint32_t height;
  uint32_t framerate;
};

static_assert(sizeof(MonitorConfiguration) == (4 * 4), "Size of MonitorConfiguration is incorrect");

struct CursorState {
  int32_t x;
  int32_t y;
  uint32_t visible;
  uint32_t width;
  uint32_t height;
  uint32_t pitch;
};

static_assert(sizeof(CursorState) == (4 * 6), "Size of MonitorConfiguration is incorrect");

MonitorClient::MonitorClient(UINT ConnectorIndex) : m_FrameBufferMutex(INVALID_HANDLE_VALUE), m_CursorBufferMutex(INVALID_HANDLE_VALUE) {
  wchar_t nameBuffer[100] = { 0 };

  PSECURITY_DESCRIPTOR securityDescriptor = nullptr;
  ConvertStringSecurityDescriptorToSecurityDescriptorA("D:(A;;0xc01f0003;;;AU)", SDDL_REVISION_1, &securityDescriptor, NULL);

  SECURITY_ATTRIBUTES securityAttributes = {
    sizeof(SECURITY_ATTRIBUTES),
    securityDescriptor,
    FALSE
  };

  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uFBMutex", ConnectorIndex);
  m_FrameBufferMutex = WRL::Wrappers::Mutex(CreateMutexW(&securityAttributes, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uNewFrameEvent", ConnectorIndex);
  m_NewFrameEvent = WRL::Wrappers::Event(CreateEventW(&securityAttributes, false, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uConfigureEvent", ConnectorIndex);
  m_ConfigureEvent = WRL::Wrappers::Event(CreateEventW(&securityAttributes, false, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uFB", ConnectorIndex);
  m_FrameBufferMapping = CreateFileMappingW(
    INVALID_HANDLE_VALUE,
    &securityAttributes,
    PAGE_READWRITE,
    0,
    MAX_FB_SIZE,
    nameBuffer
  );
  if (m_FrameBufferMapping != nullptr) {
    m_FrameBuffer = static_cast<uint8_t*>(MapViewOfFile(
      m_FrameBufferMapping,
      FILE_MAP_ALL_ACCESS,
      0,
      0,
      MAX_FB_SIZE
    ));

    if (m_FrameBuffer != nullptr) {
      MonitorConfiguration* ptr = reinterpret_cast<MonitorConfiguration*>(m_FrameBuffer);
      ptr->configured = 0;
    }
  }

  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uCursorMutex", ConnectorIndex);
  m_CursorBufferMutex = WRL::Wrappers::Mutex(CreateMutexW(&securityAttributes, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uCursorPositionUpdatedEvent", ConnectorIndex);
  m_CursorPositionUpdatedEvent = WRL::Wrappers::Event(CreateEventW(&securityAttributes, false, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uCursorImageUpdatedEvent", ConnectorIndex);
  m_CursorImageUpdatedEvent = WRL::Wrappers::Event(CreateEventW(&securityAttributes, false, false, nameBuffer));
  swprintf_s(nameBuffer, 100, L"Global\\VdMonitor%uCursor", ConnectorIndex);
  m_CursorFileMapping = CreateFileMappingW(
    INVALID_HANDLE_VALUE,
    &securityAttributes,
    PAGE_READWRITE,
    0,
    CURSOR_BUFFER_SIZE + sizeof(CursorState),
    nameBuffer
  );
  if (m_CursorFileMapping != nullptr) {
    m_CursorBuffer = static_cast<uint8_t*>(MapViewOfFile(
      m_CursorFileMapping,
      FILE_MAP_ALL_ACCESS,
      0,
      0,
      CURSOR_BUFFER_SIZE + sizeof(CursorState)
    ));
  }

  LocalFree(securityDescriptor);
}

void MonitorClient::CommitModes(uint32_t width, uint32_t height, uint32_t framerate) {
  if (m_FrameBuffer == nullptr) {
    return;
  }

  MonitorConfiguration* ptr = reinterpret_cast<MonitorConfiguration*>(m_FrameBuffer);
  ptr->configured = 1;
  ptr->width = width;
  ptr->height = height;
  ptr->framerate = framerate;

  SetEvent(m_ConfigureEvent.Get());
}

void MonitorClient::SendFrame(const uint8_t* buffer, size_t buffer_len) {
  if (m_FrameBuffer == nullptr) {
    return;
  }

  auto guard = m_FrameBufferMutex.Lock();
  // It's fine even if the user-space server crashes, so we don't check for errors.

  memcpy(m_FrameBuffer + sizeof(MonitorConfiguration), buffer, buffer_len);

  if (guard.IsLocked()) {
    guard.Unlock();
  }

  SetEvent(m_NewFrameEvent.Get());
}

void MonitorClient::UpdateCursorPosition(int32_t x, int32_t y, bool visible) {
  if (m_CursorBuffer == nullptr) {
    return;
  }
  InterlockedExchange((LONG*)m_CursorBuffer, x);
  InterlockedExchange((LONG*)m_CursorBuffer + 1, y);
  InterlockedExchange((LONG*)m_CursorBuffer + 2, visible ? 1 : 0);
  SetEvent(m_CursorPositionUpdatedEvent.Get());
}

void MonitorClient::UpdateCursorImage(uint32_t width, uint32_t height, const uint8_t* buffer, uint32_t pitch) {
  if (m_CursorBuffer == nullptr) {
    return;
  }
  auto guard = m_CursorBufferMutex.Lock();

  CursorState* ptr = reinterpret_cast<CursorState*>(m_CursorBuffer);

  ptr->width = width;
  ptr->height = height;
  ptr->pitch = pitch;

  memcpy(m_CursorBuffer + sizeof(CursorState), buffer, CURSOR_BUFFER_SIZE);

  if (guard.IsLocked()) {
    guard.Unlock();
  }
  SetEvent(m_CursorImageUpdatedEvent.Get());
}

MonitorClient::~MonitorClient() {
  if (m_FrameBuffer != nullptr) {
    UnmapViewOfFile(static_cast<LPCVOID>(m_FrameBuffer));
    m_FrameBuffer = nullptr;
  }

  if (m_FrameBufferMapping != nullptr) {
    CloseHandle(m_FrameBufferMapping);
    m_FrameBufferMapping = nullptr;
  }

  if (m_CursorBuffer != nullptr) {
    UnmapViewOfFile(static_cast<LPCVOID>(m_CursorBuffer));
    m_CursorBuffer = nullptr;
  }

  if (m_CursorFileMapping != nullptr) {
    CloseHandle(m_CursorFileMapping);
    m_FrameBufferMapping = nullptr;
  }
}