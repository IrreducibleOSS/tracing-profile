
#include "wrapper.h"

#include <fcntl.h>

#include <condition_variable>
#include <fstream>
#include <span>

#include "trace_categories.h"

// see https://perfetto.dev/docs/instrumentation/tracing-sdk
// two modes are available: System mode and in process mode. they have different
// setups. both will use their destructor to clean up.
struct TracingSessionGuard {
	// ensure the constructor of the derived class is called
	virtual ~TracingSessionGuard() {}
};

// ensures the program blocks until a connection is established with the traced
// service. basically copied from here:
// https://android.googlesource.com/platform/external/perfetto/+/sdk-release/examples/sdk/example_system_wide.cc
class SessionObserver : public perfetto::TrackEventSessionObserver {
public:
	SessionObserver() { perfetto::TrackEvent::AddSessionObserver(this); }
	~SessionObserver() override {
		perfetto::TrackEvent::RemoveSessionObserver(this);
	}
	void OnStart(const perfetto::DataSourceBase::StartArgs &) override {
		std::unique_lock<std::mutex> lock(mutex);
		cv.notify_one();
	}
	void WaitForTracingStart() {
		PERFETTO_LOG("Waiting for tracing to start...");
		std::unique_lock<std::mutex> lock(mutex);
		cv.wait(lock, [] { return perfetto::TrackEvent::IsEnabled(); });
		PERFETTO_LOG("Tracing started");
	}

private:
	std::mutex mutex;
	std::condition_variable cv;
};

// used to create a fused system wide trace
struct SdkTracingSession : TracingSessionGuard {
	SdkTracingSession() {
		perfetto::TracingInitArgs args;
		args.backends = perfetto::BackendType::kSystemBackend;
		args.enable_system_consumer = false;
		perfetto::Tracing::Initialize(args);
		perfetto::TrackEvent::Register();

		SessionObserver sessionObserver;
		sessionObserver.WaitForTracingStart();
	}
	~SdkTracingSession() override {
		perfetto::TrackEvent::Flush();
		perfetto::Tracing::Shutdown();
	}
};

// used for in-process monitoring
struct ApiTracingSession : TracingSessionGuard {
	ApiTracingSession(std::string output_file, const size_t buffer_size_kb) : output_file(std::move(output_file)) {
		perfetto::TracingInitArgs args;
		args.backends = perfetto::BackendType::kInProcessBackend;
		args.enable_system_consumer = false;
		// defaults to 256
		args.shmem_size_hint_kb = 4096;
		perfetto::Tracing::Initialize(args);
		perfetto::TrackEvent::Register();

		// by default all non debug categories are enabled in
		// TrackEventConfig
		perfetto::protos::gen::TrackEventConfig track_event_cfg;

		// https://perfetto.dev/docs/reference/trace-config-proto
		perfetto::TraceConfig cfg;
		// https://perfetto.dev/docs/concepts/buffers
		// this is probably larger than needed but the space is
		// available
		cfg.add_buffers()->set_size_kb(buffer_size_kb);

		// tells how often the producer should send data to the tracing
		// service
		cfg.set_flush_period_ms(2000);

		auto *ds_cfg = cfg.add_data_sources()->mutable_config();
		ds_cfg->set_name("track_event");
		ds_cfg->set_track_event_config_raw(
		    track_event_cfg.SerializeAsString());

		std::unique_ptr<perfetto::TracingSession> tracing_session(
		    perfetto::Tracing::NewTrace(
			perfetto::BackendType::kInProcessBackend));
		this->tracing_session = std::move(tracing_session);

		this->tracing_session->Setup(cfg);
		this->tracing_session->StartBlocking();
	}

	~ApiTracingSession() {
		this->tracing_session->FlushBlocking(100);
		this->tracing_session->StopBlocking();
		std::vector<char> trace_data(
		    tracing_session->ReadTraceBlocking());

		std::ofstream output;
		output.open(output_file.c_str(),
			    std::ios::out | std::ios::binary | std::ios::trunc);
		output.write(&trace_data[0], trace_data.size());
		output.close();
	}

private:
	std::unique_ptr<perfetto::TracingSession> tracing_session;
	std::string output_file;
};

void *init_perfetto(uint32_t backend, const char* output_file, size_t buffer_size_kb) {
	assert(output_file);
	
	auto backend_type = static_cast<perfetto::BackendType>(backend);
	TracingSessionGuard *ptr = nullptr;
	if (backend_type == perfetto::BackendType::kSystemBackend) {
		ptr = new SdkTracingSession();
	} else {
		// warning: silently refuses custom backend
		ptr = new ApiTracingSession(output_file, buffer_size_kb);
	}
	auto p = (void *)(ptr);
	return p;
}

void deinit_perfetto(void *guard) {
	assert(guard);

	const auto* p = reinterpret_cast<TracingSessionGuard*>(guard);
	delete p;
}

void create_event(EventType event_type, const char* category, const char* name, const uint64_t* track_id, const PerfettoEventArg* args, size_t arg_count) {
	assert(name);
	assert(args || arg_count == 0);
	
	auto set_props = [&](perfetto::EventContext ctx) {
		for (const auto& arg: std::span{args, arg_count}) {
			switch (arg.type) {
				case ArgType::FlowID:
					ctx.event()->add_flow_ids(arg.data.u64);
					break;
				case ArgType::StringKeyValue:
					ctx.AddDebugAnnotation(arg.data.string_key_value.key, arg.data.string_key_value.value);
					break;
				case ArgType::F64KeyValue:
					ctx.AddDebugAnnotation(arg.data.f64_key_value.key, arg.data.f64_key_value.value);
					break;
				case ArgType::I64KeyValue:
					ctx.AddDebugAnnotation(arg.data.i64_key_value.key, arg.data.i64_key_value.value);
					break;
				case ArgType::U64KeyValue:
					ctx.AddDebugAnnotation(arg.data.u64_key_value.key, arg.data.u64_key_value.value);
					break;
				case ArgType::BoolKeyValue:
					ctx.AddDebugAnnotation(arg.data.bool_key_value.key, arg.data.bool_key_value.value);
					break;
			}
		}
	};

	auto name_str = perfetto::DynamicString{name};
	auto category_str = category ? perfetto::DynamicCategory{category} : perfetto::DynamicCategory{"default"};

	if (event_type == EventType::Span) {
		if (track_id) {
			TRACE_EVENT_BEGIN(category_str, name_str, perfetto::Track(*track_id), set_props);
		} else {
			TRACE_EVENT_BEGIN(category_str, name_str, set_props);
		}
	} else if (event_type == EventType::Instant) {
		if (track_id) {
			TRACE_EVENT_INSTANT(category_str, name_str, perfetto::Track(*track_id), set_props);
		} else {
			TRACE_EVENT_INSTANT(category_str, name_str, set_props);
		}
	}
}

void destroy_event(const char* category, const uint64_t* track_id) {
	if (category) {
		perfetto::DynamicCategory category_name{category};
		if (track_id) {
			TRACE_EVENT_END(category_name, perfetto::Track(*track_id));
		} else {
			TRACE_EVENT_END(category_name);
		}
	} else {
		if (track_id) {
			TRACE_EVENT_END("default", perfetto::Track(*track_id));
		} else {
			TRACE_EVENT_END("default");
		}
	}
}

namespace {

template <typename T>
void update_counter(const char* category, const char* name, const char* unit, const bool is_incremental, const T value) {
	assert(name);
	
	perfetto::CounterTrack memory_track = perfetto::CounterTrack(name);
	if (unit) {
		memory_track.set_unit_name(unit);
	}
	memory_track.set_is_incremental(is_incremental);
	if (category) {
		perfetto::DynamicCategory category_name{category};
		TRACE_COUNTER(category_name, memory_track, value);
	} else {
		TRACE_COUNTER("default", memory_track, value);
	}
}

} // namespace

void update_counter_u64(const char* category, const char* name, const char* unit, const bool is_incremental, const uint64_t value) {
	update_counter(category, name, unit, is_incremental, value);
}

void update_counter_f64(const char* category, const char* name, const char* unit, const bool is_incremental, const double value) {
	update_counter(category, name, unit, is_incremental, value);
}