
#include "wrapper.h"

#include <fcntl.h>

#include <condition_variable>
#include <fstream>

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
	ApiTracingSession() {
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
		cfg.add_buffers()->set_size_kb(50 * 1024);

		// tells how often the producer should send data to the tracing
		// service
		cfg.set_flush_period_ms(5000);

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

		const char *output_file = std::getenv("PERFETTO_OUTPUT");
		if (output_file == nullptr) {
			output_file = "tracing.perfetto-trace";
		}
		std::ofstream output;
		output.open(output_file,
			    std::ios::out | std::ios::binary | std::ios::trunc);
		output.write(&trace_data[0], trace_data.size());
		output.close();
	}

private:
	std::unique_ptr<perfetto::TracingSession> tracing_session;
};

void *init_perfetto(uint32_t backend) {
	auto backend_type = static_cast<perfetto::BackendType>(backend);
	TracingSessionGuard *ptr = nullptr;
	if (backend_type == perfetto::BackendType::kSystemBackend) {
		ptr = new SdkTracingSession();
	} else {
		// warning: silently refuses custom backend
		ptr = new ApiTracingSession();
	}
	auto p = (void *)(ptr);
	return p;
}

void deinit_perfetto(void *guard) {
	const auto* p = reinterpret_cast<TracingSessionGuard*>(guard);
	delete p;
}
