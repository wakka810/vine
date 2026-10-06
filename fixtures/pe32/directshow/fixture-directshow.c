typedef unsigned char BYTE;
typedef unsigned short WORD;
typedef unsigned int ULONG;
typedef int LONG;
typedef int HRESULT;
typedef long long REFERENCE_TIME;
typedef struct { ULONG a; WORD b, c; BYTE d[8]; } GUID;
typedef struct { const void *const *vtable; void *owner; } Face;
typedef struct { LONG left, top, right, bottom; } Rect;
typedef struct {
  ULONG size; LONG width, height; WORD planes, bits;
  ULONG compression, image_bytes; LONG xppm, yppm; ULONG colors, important;
} BitmapHeader;
typedef struct {
  Rect source, target; ULONG bitrate, error_rate; REFERENCE_TIME frame_time;
  BitmapHeader bitmap;
} VideoInfo;
typedef struct {
  GUID major, subtype; LONG fixed, compressed; ULONG sample_bytes;
  GUID format; void *unknown; ULONG format_bytes; BYTE *format_data;
} MediaType;
typedef struct { LONG buffers, bytes, alignment, prefix; } AllocatorProperties;
_Static_assert(sizeof(void *) == 4, "PE32 fixture");
_Static_assert(sizeof(MediaType) == 72 && sizeof(VideoInfo) == 88, "DirectShow ABI");

__declspec(dllimport) HRESULT __stdcall CoInitialize(void *);
__declspec(dllimport) HRESULT __stdcall CoCreateInstance(const GUID *, void *, ULONG, const GUID *, void **);
__declspec(dllimport) void *__stdcall CoTaskMemAlloc(ULONG);
__declspec(dllimport) void __stdcall CoTaskMemFree(void *);
__declspec(dllimport) void __stdcall Sleep(ULONG);

#define S_OK 0
#define S_FALSE 1
#define E_POINTER ((HRESULT)0x80004003u)
#define E_NOINTERFACE ((HRESULT)0x80004002u)
#define E_NOTIMPL ((HRESULT)0x80004001u)
#define E_INVALIDARG ((HRESULT)0x80070057u)
#define VFW_NOT_CONNECTED ((HRESULT)0x80040209u)
#define VFW_WRONG_STATE ((HRESULT)0x80040227u)
#define VT(function) ((const void *)(function))
#define CALL0(p,s) (((HRESULT (__stdcall *)(void *))((Face *)(p))->vtable[s])((p)))
#define CALL1(p,s,a) (((HRESULT (__stdcall *)(void *,ULONG))((Face *)(p))->vtable[s])((p),(ULONG)(a)))
#define CALL2(p,s,a,b) (((HRESULT (__stdcall *)(void *,ULONG,ULONG))((Face *)(p))->vtable[s])((p),(ULONG)(a),(ULONG)(b)))
#define CALL3(p,s,a,b,c) (((HRESULT (__stdcall *)(void *,ULONG,ULONG,ULONG))((Face *)(p))->vtable[s])((p),(ULONG)(a),(ULONG)(b),(ULONG)(c)))
#define CALL4(p,s,a,b,c,d) (((HRESULT (__stdcall *)(void *,ULONG,ULONG,ULONG,ULONG))((Face *)(p))->vtable[s])((p),(ULONG)(a),(ULONG)(b),(ULONG)(c),(ULONG)(d)))
#define DSHOW_GUID(low) {0x56a86800u + (low),0x0ad4,0x11ce,{0xb0,0x3a,0x00,0x20,0xaf,0x0b,0xa7,0x70}}
static const GUID iid_unknown = {0,0,0,{0xc0,0,0,0,0,0,0,0x46}};
static const GUID iid_graph = DSHOW_GUID(0xa9), iid_control = DSHOW_GUID(0xb1), iid_event = DSHOW_GUID(0xc0);
static const GUID iid_event_sink = DSHOW_GUID(0xa2);
static const GUID iid_filter = DSHOW_GUID(0x95), iid_pin = DSHOW_GUID(0x91), iid_enum = DSHOW_GUID(0x92);
static const GUID iid_reader = DSHOW_GUID(0xaa), iid_mem = DSHOW_GUID(0x9d);
static const GUID clsid_graph = {0xe436ebb3,0x524f,0x11ce,{0x9f,0x53,0,0x20,0xaf,0x0b,0xa7,0x70}};
static const GUID iid_unavailable = {0x8e6ba401,0x1234,0x4567,{0x89,0xab,0xcd,0xef,1,2,3,4}};
static const GUID major_video = {0x73646976,0,0x10,{0x80,0,0,0xaa,0,0x38,0x9b,0x71}};
static const GUID rgb32 = {0xe436eb7e,0x524f,0x11ce,{0x9f,0x53,0,0x20,0xaf,0x0b,0xa7,0x70}};
static const GUID video_info = {0x05589f80,0xc356,0x11ce,{0xbf,1,0,0xaa,0,0x55,0x59,0x5a}};

/* Generated MPEG-1: 16x16, 25 fps, two I pictures with one intra macroblock.
 * Six DC differences are zero and each block ends immediately (no AC terms).
 * This contains no extracted media. Actual Wasm decoder output is checked below. */
static const BYTE video[] = {
  0x00,0x00,0x01,0xb3,0x01,0x00,0x10,0x13,0x00,0x00,0x60,0xa0,
  0x00,0x00,0x01,0x00,0x00,0x0f,0xff,0xf8,
  0x00,0x00,0x01,0x01,0x43,0x94,0xa5,0x22,0x20,
  0x00,0x00,0x01,0x00,0x00,0x4f,0xff,0xf8,
  0x00,0x00,0x01,0x01,0x43,0x94,0xa5,0x22,0x20,
  0x00,0x00,0x01,0xb7
};

typedef struct {
  Face filter, pin, stream, enumerator;
  ULONG refs, pin_refs, enum_refs, enum_position;
  LONG is_source, state;
  void *graph, *peer, *allocator;
  ULONG joins, departures, pauses, runs, stops, reads, bytes_read;
  ULONG notifications, frames, end_of_stream, failures;
  ULONG window_notifications;
  REFERENCE_TIME last_end;
} Filter;
static Filter source, sink;
static int same_guid(const GUID *a, const GUID *b) {
  ULONG i; const BYTE *x = (const BYTE *)a, *y = (const BYTE *)b;
  if (!a || !b) return 0;
  for (i = 0; i < sizeof(GUID); ++i) if (x[i] != y[i]) return 0;
  return 1;
}
static Filter *object(void *self) { return (Filter *)((Face *)self)->owner; }
static ULONG __stdcall filter_add(void *self) { return ++object(self)->refs; }
static ULONG __stdcall filter_release(void *self) { Filter *f = object(self); if (!f->refs) { ++f->failures; return 0; } return --f->refs; }
static HRESULT __stdcall filter_query(void *self, const GUID *iid, void **out) {
  if (!out) return E_POINTER; *out = 0;
  if (!same_guid(iid,&iid_unknown) && !same_guid(iid,&iid_filter)) return E_NOINTERFACE;
  *out = &object(self)->filter; filter_add(*out); return S_OK;
}
static ULONG __stdcall pin_add(void *self) { return ++object(self)->pin_refs; }
static ULONG __stdcall pin_release(void *self) {
  Filter *f = object(self);
  if (!f->pin_refs) { ++f->failures; return 0; }
  if (--f->pin_refs == 0 && f->allocator) { CALL0(f->allocator,2); f->allocator = 0; }
  return f->pin_refs;
}
static HRESULT __stdcall pin_query(void *self, const GUID *iid, void **out) {
  Filter *f = object(self); if (!out) return E_POINTER; *out = 0;
  if (same_guid(iid,&iid_unknown) || same_guid(iid,&iid_pin)) *out = &f->pin;
  else if ((f->is_source && same_guid(iid,&iid_reader)) || (!f->is_source && same_guid(iid,&iid_mem))) *out = &f->stream;
  else return E_NOINTERFACE;
  pin_add(*out); return S_OK;
}
static HRESULT __stdcall get_class(void *self, GUID *out) { (void)self; if (!out) return E_POINTER; *out = iid_unavailable; return S_OK; }
static HRESULT __stdcall stop(void *self) { Filter *f = object(self); f->state = 0; ++f->stops; return S_OK; }
static HRESULT __stdcall pause_filter(void *self) { Filter *f = object(self); f->state = 1; ++f->pauses; return S_OK; }
static HRESULT __stdcall run_filter(void *self, REFERENCE_TIME start) { Filter *f = object(self); if (start < 0) return E_INVALIDARG; f->state = 2; ++f->runs; return S_OK; }
static HRESULT __stdcall get_state(void *self, ULONG wait, LONG *out) { (void)wait; if (!out) return E_POINTER; *out = object(self)->state; return S_OK; }
static HRESULT __stdcall set_clock(void *self, void *clock) { (void)self; return clock ? E_NOTIMPL : S_OK; }
static HRESULT __stdcall get_clock(void *self, void **out) { (void)self; if (!out) return E_POINTER; *out = 0; return S_OK; }
static HRESULT __stdcall enum_query(void *self, const GUID *iid, void **out) {
  if (!out) return E_POINTER; *out = 0;
  if (!same_guid(iid,&iid_unknown) && !same_guid(iid,&iid_enum)) return E_NOINTERFACE;
  *out = &object(self)->enumerator; ++object(self)->enum_refs; return S_OK;
}
static ULONG __stdcall enum_add(void *self) { return ++object(self)->enum_refs; }
static ULONG __stdcall enum_release(void *self) { Filter *f = object(self); if (!f->enum_refs) { ++f->failures; return 0; } if (--f->enum_refs == 0) filter_release(&f->filter); return f->enum_refs; }
static HRESULT __stdcall enum_next(void *self, ULONG count, void **out, ULONG *fetched) {
  Filter *f = object(self); if (!out || (!fetched && count != 1)) return E_POINTER;
  if (fetched) *fetched = 0; if (!count) return S_OK;
  *out = 0; if (f->enum_position) return S_FALSE;
  *out = &f->pin; pin_add(*out); f->enum_position = 1; if (fetched) *fetched = 1;
  return count == 1 ? S_OK : S_FALSE;
}
static HRESULT __stdcall enum_skip(void *self, ULONG count) { Filter *f=object(self); if (!count) return S_OK; if (f->enum_position) return S_FALSE; f->enum_position=1; return count==1 ? S_OK:S_FALSE; }
static HRESULT __stdcall enum_reset(void *self) { object(self)->enum_position=0; return S_OK; }
static HRESULT __stdcall enum_clone(void *self, void **out) { (void)self; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall enum_pins(void *self, void **out) {
  Filter *f=object(self); if (!out) return E_POINTER; *out=0;
  if (f->enum_refs) return E_NOTIMPL;
  f->enum_position=0; f->enum_refs=1; filter_add(self); *out=&f->enumerator; return S_OK;
}
static HRESULT __stdcall find_pin(void *self, const WORD *name, void **out) { (void)self; (void)name; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall filter_info(void *self, void *out) { (void)self; (void)out; return E_NOTIMPL; }
static HRESULT __stdcall join_graph(void *self, void *graph, const WORD *name) {
  Filter *f=object(self); (void)name;
  if (!graph && f->graph && !f->is_source) {
    void *notify=0, *events=0;
    LONG code=0x12345678, first=0x23456789, second=0x3456789a;
    /* A renderer reports its removal while its filter and graph are still live. */
    if (CALL2(f->graph,0,&iid_event_sink,&notify)!=S_OK || !notify) return E_NOINTERFACE;
    HRESULT hr=CALL3(notify,3,21,self,0);
    if (hr==S_OK) {
      ++f->window_notifications;
      if (CALL2(f->graph,0,&iid_event,&events)!=S_OK || !events) hr=E_NOINTERFACE;
      else {
        /* EC_WINDOW_DESTROYED has default handling and is not an application event. */
        if (CALL4(events,8,&code,&first,&second,0)!=(HRESULT)0x80004004u ||
            code!=0x12345678 || first!=0x23456789 || second!=0x3456789a) hr=E_INVALIDARG;
        CALL0(events,2);
      }
    }
    CALL0(notify,2);
    if (hr!=S_OK) { ++f->failures; return hr; }
  }
  f->graph=graph;
  if (graph) ++f->joins; else ++f->departures;
  return S_OK;
}
static HRESULT __stdcall vendor_info(void *self, WORD **out) { (void)self; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall pin_connect(void *self, void *peer, const MediaType *media) { (void)self; (void)peer; (void)media; return E_NOTIMPL; }
static HRESULT __stdcall query_accept(void *self, const MediaType *media) {
  Filter *f=object(self); VideoInfo *info;
  if (!media) return E_POINTER;
  if (f->is_source) return E_NOTIMPL;
  if (!same_guid(&media->major,&major_video) || !same_guid(&media->subtype,&rgb32) || !same_guid(&media->format,&video_info)) return S_FALSE;
  if (!media->format_data || media->format_bytes < sizeof(VideoInfo)) return S_FALSE;
  info=(VideoInfo *)media->format_data;
  return info->bitmap.size==40 && info->bitmap.width==16 && info->bitmap.height==-16 &&
    info->bitmap.planes==1 && info->bitmap.bits==32 && info->bitmap.compression==0 &&
    info->frame_time==400000 ? S_OK:S_FALSE;
}
static HRESULT __stdcall receive_connection(void *self, void *peer, const MediaType *media) {
  Filter *f=object(self); LONG direction=-1;
  if (!peer || !media) return E_POINTER;
  if (f->is_source || f->state || f->peer) return VFW_WRONG_STATE;
  if (query_accept(self,media)!=S_OK || CALL1(peer,9,&direction)!=S_OK || direction!=1) return E_INVALIDARG;
  CALL0(peer,1); f->peer=peer; return S_OK;
}
static HRESULT __stdcall disconnect(void *self) { Filter *f=object(self); if (f->state) return VFW_WRONG_STATE; if (!f->peer) return S_FALSE; CALL0(f->peer,2); f->peer=0; return S_OK; }
static HRESULT __stdcall connected_to(void *self, void **out) { Filter *f=object(self); if (!out) return E_POINTER; *out=0; if (!f->peer) return VFW_NOT_CONNECTED; *out=f->peer; CALL0(*out,1); return S_OK; }
static HRESULT __stdcall connection_media(void *self, MediaType *out) { (void)self; (void)out; return E_NOTIMPL; }
static HRESULT __stdcall pin_info(void *self, void *out) { (void)self; (void)out; return E_NOTIMPL; }
static HRESULT __stdcall direction(void *self, LONG *out) { if (!out) return E_POINTER; *out=object(self)->is_source ? 1:0; return S_OK; }
static HRESULT __stdcall query_id(void *self, WORD **out) { (void)self; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall enum_media(void *self, void **out) { (void)self; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall internal_connections(void *self, void **out, ULONG *count) { (void)self; (void)out; (void)count; return E_NOTIMPL; }
static HRESULT __stdcall end_stream(void *self) { ++object(self)->end_of_stream; return S_OK; }
static HRESULT __stdcall begin_flush(void *self) { (void)self; return S_OK; }
static HRESULT __stdcall end_flush(void *self) { (void)self; return S_OK; }
static HRESULT __stdcall new_segment(void *self, REFERENCE_TIME start, REFERENCE_TIME end, double rate) { (void)self; (void)start; (void)end; (void)rate; return S_OK; }
static HRESULT __stdcall get_allocator(void *self, void **out) { Filter *f=object(self); if (!out) return E_POINTER; *out=f->allocator; if (!*out) return VFW_NOT_CONNECTED; CALL0(*out,1); return S_OK; }
static HRESULT __stdcall notify_allocator(void *self, void *allocator, LONG readonly) {
  Filter *f=object(self); AllocatorProperties properties;
  if (!allocator) return E_POINTER;
  if (!readonly || CALL1(allocator,4,&properties)!=S_OK || properties.buffers<1 || properties.bytes<1024 || properties.alignment<1 || properties.prefix<0) return E_INVALIDARG;
  CALL0(allocator,1); if (f->allocator) CALL0(f->allocator,2); f->allocator=allocator; ++f->notifications; return S_OK;
}
static HRESULT __stdcall allocator_requirements(void *self, AllocatorProperties *out) { (void)self; (void)out; return E_NOTIMPL; }
static HRESULT __stdcall receive(void *self, void *sample) {
  Filter *f=object(self); BYTE *bytes=0; REFERENCE_TIME start=-1,end=-1;
  ULONG i; void *dynamic=(void *)1;
  if (!sample) return E_POINTER;
  if (f->state!=2 || !f->peer || !f->allocator) return VFW_WRONG_STATE;
  if (CALL1(sample,3,&bytes)!=S_OK || !bytes || CALL0(sample,4)<1024 || CALL0(sample,11)!=1024 ||
      CALL2(sample,5,&start,&end)!=S_OK || start!=(REFERENCE_TIME)f->frames*400000 || end-start!=400000 ||
      CALL0(sample,7)!=S_OK || CALL0(sample,9)!=S_FALSE || CALL0(sample,15)!=S_FALSE ||
      CALL1(sample,13,&dynamic)!=S_FALSE || dynamic!=0) { ++f->failures; return E_INVALIDARG; }
  for (i=0;i<256;++i) {
    if (bytes[i*4]!=129 || bytes[i*4+1]!=129 || bytes[i*4+2]!=129 || bytes[i*4+3]!=255) { ++f->failures; return E_INVALIDARG; }
  }
  f->last_end=end; ++f->frames; return S_OK;
}
static HRESULT __stdcall receive_multiple(void *self, void **samples, LONG count, LONG *processed) {
  LONG i; HRESULT hr; if (!samples || !processed || count<0) return E_INVALIDARG; *processed=0;
  for(i=0;i<count;++i) { hr=receive(self,samples[i]); if(hr!=S_OK) return hr; ++*processed; }
  return S_OK;
}
static HRESULT __stdcall receive_can_block(void *self) { (void)self; return S_FALSE; }
static HRESULT __stdcall request_allocator(void *self, void *preferred, AllocatorProperties *properties, void **out) { (void)self; (void)preferred; (void)properties; if (!out) return E_POINTER; *out=0; return E_NOTIMPL; }
static HRESULT __stdcall request_sample(void *self, void *sample, ULONG user) { (void)self; (void)sample; (void)user; return E_NOTIMPL; }
static HRESULT __stdcall wait_sample(void *self, ULONG timeout, void **sample, ULONG *user) { (void)self; (void)timeout; if (!sample || !user) return E_POINTER; *sample=0; *user=0; return E_NOTIMPL; }
static HRESULT __stdcall sync_aligned(void *self, void *sample) { (void)self; (void)sample; return E_NOTIMPL; }
static HRESULT __stdcall sync_read(void *self, REFERENCE_TIME offset, LONG length, BYTE *out) {
  Filter *f=object(self); ULONG i;
  if (!out) return E_POINTER;
  if (offset<0 || length<0 || offset>(REFERENCE_TIME)sizeof(video) || (REFERENCE_TIME)length>(REFERENCE_TIME)sizeof(video)-offset) return E_INVALIDARG;
  for(i=0;i<(ULONG)length;++i) out[i]=video[(ULONG)offset+i];
  ++f->reads; f->bytes_read+=(ULONG)length; return S_OK;
}
static HRESULT __stdcall reader_length(void *self, REFERENCE_TIME *total, REFERENCE_TIME *available) { (void)self; if (!total || !available) return E_POINTER; *total=sizeof(video); *available=sizeof(video); return S_OK; }

static const void *const filter_vtable[] = {
  VT(filter_query),VT(filter_add),VT(filter_release),VT(get_class),VT(stop),VT(pause_filter),VT(run_filter),VT(get_state),
  VT(set_clock),VT(get_clock),VT(enum_pins),VT(find_pin),VT(filter_info),VT(join_graph),VT(vendor_info)
};
static const void *const pin_vtable[] = {
  VT(pin_query),VT(pin_add),VT(pin_release),VT(pin_connect),VT(receive_connection),VT(disconnect),VT(connected_to),VT(connection_media),
  VT(pin_info),VT(direction),VT(query_id),VT(query_accept),VT(enum_media),VT(internal_connections),VT(end_stream),VT(begin_flush),VT(end_flush),VT(new_segment)
};
static const void *const enum_vtable[] = {VT(enum_query),VT(enum_add),VT(enum_release),VT(enum_next),VT(enum_skip),VT(enum_reset),VT(enum_clone)};
static const void *const reader_vtable[] = {VT(pin_query),VT(pin_add),VT(pin_release),VT(request_allocator),VT(request_sample),VT(wait_sample),VT(sync_aligned),VT(sync_read),VT(reader_length),VT(begin_flush),VT(end_flush)};
static const void *const mem_vtable[] = {VT(pin_query),VT(pin_add),VT(pin_release),VT(get_allocator),VT(notify_allocator),VT(allocator_requirements),VT(receive),VT(receive_multiple),VT(receive_can_block)};
static void initialize(Filter *f, LONG is_source) {
  f->filter.vtable=filter_vtable; f->filter.owner=f; f->pin.vtable=pin_vtable; f->pin.owner=f;
  f->stream.vtable=is_source ? reader_vtable:mem_vtable; f->stream.owner=f;
  f->enumerator.vtable=enum_vtable; f->enumerator.owner=f;
  f->refs=1; f->pin_refs=1; f->is_source=is_source;
}

int __cdecl fixture_entry(void) {
  void *graph=0,*control=0,*events=0,*identity=0,*other_identity=0,*unknown=(void *)1;
  void *empty; volatile BYTE *format_bytes; ULONG byte;
  LONG event_code=0,event_a=0,event_b=0,state=-1; ULONG attempts; HRESULT hr;
  static const WORD sink_name[]={'R','G','B',' ','s','i','n','k',0};
  static const WORD source_name[]={'E','S',' ','s','o','u','r','c','e',0};
  /* Task memory works before COM apartment initialization. The guest CPU must
   * be able to write and reread the same 88-byte format used by VideoInfo. */
  empty=CoTaskMemAlloc(0); format_bytes=(BYTE *)CoTaskMemAlloc(sizeof(VideoInfo));
  if(!empty || !format_bytes || ((ULONG)empty & 7) || ((ULONG)format_bytes & 7)) return 23;
  for(byte=0;byte<sizeof(VideoInfo);++byte) format_bytes[byte]=(BYTE)(byte ^ 0x5a);
  for(byte=0;byte<sizeof(VideoInfo);++byte) if(format_bytes[byte]!=(BYTE)(byte ^ 0x5a)) return 24;
  CoTaskMemFree(empty); CoTaskMemFree((void *)format_bytes); CoTaskMemFree(0);
  initialize(&source,1); initialize(&sink,0);
  if(CoInitialize(0)<0) return 1;
  if(CoCreateInstance(&clsid_graph,0,1,&iid_unavailable,&unknown)!=E_NOINTERFACE || unknown) return 2;
  if(CoCreateInstance(&clsid_graph,0,1,&iid_graph,&graph)!=S_OK || !graph) return 3;
  unknown=(void *)1;
  if(CALL2(graph,0,&iid_unavailable,&unknown)!=E_NOINTERFACE || unknown) return 4;
  if(CALL2(graph,0,&iid_control,&control)!=S_OK || CALL2(graph,0,&iid_event,&events)!=S_OK) return 5;
  if(CALL2(graph,0,&iid_unknown,&identity)!=S_OK || CALL2(control,0,&iid_unknown,&other_identity)!=S_OK || identity!=other_identity) return 6;
  CALL0(identity,2); CALL0(other_identity,2);
  if(CALL2(graph,3,&sink.filter,sink_name)!=S_OK || CALL2(graph,3,&source.filter,source_name)!=S_OK) return 7;
  if(source.refs<2 || sink.refs<2 || source.joins!=1 || sink.joins!=1) return 8;
  if(CALL1(graph,12,&source.pin)!=S_OK) return 9;
  if(!source.reads || source.bytes_read<sizeof(video) || !sink.peer || !sink.notifications) return 10;
  if(CALL0(control,8)!=S_OK || source.state!=1 || sink.state!=1 || !source.pauses || !sink.pauses) return 11;
  if(CALL0(control,7)!=S_OK || !source.runs || !sink.runs) return 22;
  if(CALL2(control,10,0,&state)<0 || state!=2) return 12;
  for(attempts=0;attempts<1000;++attempts) {
    hr=CALL4(events,8,&event_code,&event_a,&event_b,0);
    if(hr==S_OK) break;
    if(hr!=(HRESULT)0x80004004u) return 13;
    Sleep(5);
  }
  if(attempts==1000 || event_code!=1) return 14;
  if(sink.frames!=2 || sink.last_end!=800000 || sink.failures || source.failures) return 15;
  if(CALL3(events,12,event_code,event_a,event_b)!=S_OK) return 16;
  if(CALL0(control,9)!=S_OK || source.state || sink.state || !source.stops || !sink.stops) return 17;
  CALL0(events,2); CALL0(control,2); CALL0(graph,2);
  if(source.refs!=1 || sink.refs!=1 || source.enum_refs || sink.enum_refs || !source.departures || !sink.departures) return 18;
  if(sink.window_notifications!=1 || source.window_notifications || sink.failures || source.failures) return 25;
  /* Release the fixture's own initial references after graph teardown. */
  if(sink.peer && disconnect(&sink.pin)!=S_OK) return 19;
  if(pin_release(&source.pin)!=0 || pin_release(&sink.pin)!=0) return 20;
  if(filter_release(&source.filter)!=0 || filter_release(&sink.filter)!=0) return 21;
  return 0;
}
