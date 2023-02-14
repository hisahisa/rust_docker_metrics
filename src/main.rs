use std::sync::mpsc::channel;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::{BufReader, prelude::*};
use std::net::TcpListener;
use std::fs::File;
use threadpool::ThreadPool;
use subprocess::{Popen, PopenConfig, Redirection};
use serde::Serialize;
use serde_json;
use chrono::{NaiveDateTime, Duration, Local};

const THRESHOLD: usize = 360;
const THRESHOLD_ELEMENT: usize = 20;
const UNIT: [&'static str; 3] = ["KiB", "MiB", "GiB"];
const COLOR: [&'static str; 10] = [
    "#cc3300", "#3300ff", "#ffff00", "#006633", "#00ff99",
    "#00cc00", "#ffcccc", "#0000cc", "#666699", "#ffbf7f"];

static mut COLOR_INDEX:usize = 9;
unsafe fn line_color<'a>() ->&'a str {
    if COLOR_INDEX > 8 {
        COLOR_INDEX = 0
    } else {
        COLOR_INDEX += 1
    }
    COLOR[COLOR_INDEX]
}

#[derive(Serialize, Debug, Clone)]
struct ResponseParent {
    label_date_time: VecDeque<String>,
    labels: VecDeque<String>,
    limit: Option<String>,
    datasets: VecDeque<ResponseChild>,
}

impl ResponseParent {
    fn new () -> ResponseParent {
        ResponseParent{
            label_date_time: VecDeque::from_iter(vec!["".to_string(); THRESHOLD]),
            labels: VecDeque::from_iter(vec!["".to_string(); THRESHOLD]),
            limit: None,
            datasets: VecDeque::from([])
        }
    }
    fn up(&mut self) {
        self.labels.pop_front();
        self.labels.push_back(Local::now().format( "%H:%M:%S").to_string());
        self.label_date_time.pop_front();
        self.label_date_time.push_back(Local::now().format( "%Y/%m/%d %H:%M:%S").to_string())
    }
}

#[derive(Serialize, Debug, Clone)]
struct ResponseChild {
    id: String,
    label: String,
    border_color: String,
    data: VecDeque<Option<f64>>,
    data_tmp: VecDeque<Option<f64>>,
    data_cpu: VecDeque<Option<f64>>,
    data_cpu_tmp: VecDeque<Option<f64>>
}

impl ResponseChild {
    fn new (id_: String, label_: String, border_color: String) -> ResponseChild {
        ResponseChild{
            id: id_,
            label: label_,
            border_color,
            data: VecDeque::from_iter(vec![None; THRESHOLD]),
            data_tmp: VecDeque::from_iter(vec![]),
            data_cpu: VecDeque::from_iter(vec![None; THRESHOLD]),
            data_cpu_tmp: VecDeque::from_iter(vec![])
        }
    }
    fn init(&mut self, val: Option<f64>, cpu_val: Option<f64>) {
        self.data_tmp.push_back(val);
        self.data_cpu_tmp.push_back(cpu_val);
    }
    fn up_none(&mut self) {
        self.data.pop_front();
        self.data.push_back(None);
        self.data_cpu.pop_front();
        self.data_cpu.push_back(None);
    }
    fn up_val(&mut self, val: Option<f64>, cpu_val: Option<f64>, counter: &usize) {
        self.data_tmp.pop_back();
        self.data_tmp.push_back(val);
        self.data_cpu_tmp.pop_back();
        self.data_cpu_tmp.push_back(cpu_val);
        if counter % THRESHOLD_ELEMENT == 0 {
            self.data.pop_back();
            self.data_cpu.pop_back();
            let val = self.data_tmp.iter().all(|a| a.is_none());
            if val {
                self.data.push_back(None);
                self.data_cpu.push_back(None);
            } else {
                let f = |a:f64, b: &Option<f64>| a.max(b.unwrap_or(0.0));
                let mem_max = self.data_tmp.iter()
                    .fold(f64::NAN, f);
                let cpu_max = self.data_cpu_tmp.iter()
                    .fold(f64::NAN, f);
                self.data.push_back(Some(mem_max));
                self.data_cpu.push_back(Some(cpu_max));
            }
            self.data_tmp.clear();
            self.data_cpu_tmp.clear();
        }
    }
}

fn main() {
    let address = "127.0.0.1:7878";
    let command = "docker stats";
    let listener = TcpListener::bind(address).unwrap();

    let command_vec = command.split(" ").collect::<Vec<&str>>();
    let popen = Popen::create(&command_vec, PopenConfig {
        stdout: Redirection::Pipe,
        ..Default::default()
    }).unwrap();

    let (tx, rx) = channel();
    let pool = ThreadPool::new(1);
    let mut send_vec:Vec<String> = vec![];
    let mut counter:usize = 0;
    pool.execute(move || {
        let mut parent_res = ResponseParent::new();
        let stdout_file = popen.stdout.as_ref().unwrap();
        let tx = tx.clone();
        for result in BufReader::new(stdout_file).lines() {
            let result_string = result.unwrap();
            if result_string.starts_with("\u{1b}") {
                if send_vec.len() > 0 {
                    counter += 1;
                    let error_message = "channel will be there waiting for the pool";
                    response_object(&mut parent_res, send_vec, &counter);
                    if counter % THRESHOLD_ELEMENT == 0 {
                        parent_res.up();
                        tx.send(parent_res.clone()).expect(error_message);
                    }
                    send_vec = vec![];  // initialize
                }
                send_vec.push(result_string[7..].to_string());
            } else {
                send_vec.push(result_string[..].to_string());
            }
        }
    });

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        let mut buffer = [0; 1024];
        stream.read(&mut buffer).unwrap();
        let metrics = b"GET /metrics HTTP/1.1\r\n";

        if buffer.starts_with(metrics) {
            for mut my_response in rx.iter() {

                fn is_time_range(res: & ResponseParent) -> bool {
                    let label_dt = res.label_date_time.back().unwrap().as_str();
                    let fmt_str = "%Y/%m/%d %H:%M:%S";
                    let ndt = NaiveDateTime::parse_from_str(label_dt, fmt_str).unwrap();
                    let ndt_now = Local::now().naive_local();
                    let duration: Duration = ndt_now - ndt;
                    duration.num_seconds() <= 60  // 遅延 60sec以上はskip
                }

                if is_time_range(&my_response) {
                    // println!("my_response  {:?}", my_response);
                    println!("my_response time {:?}", my_response.labels.back().unwrap());
                    my_response.label_date_time.clear();
                    let serialized_json = serde_json::to_string(&my_response).unwrap();
                    let json_size = serialized_json.len();
                    let (header_first, header_last) = header_tuple();
                    let response = format!("{}{}{}{}", header_first, json_size,
                                           header_last, serialized_json);
                    stream.write(response.as_bytes()).unwrap();
                    break;
                } else {
                    println!("skipping my_response  {:?}", my_response);
                    continue;
                }

            }
        } else {
            let mut file = File::open("chart.html").unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            let response = format!("{}{}", "HTTP/1.1 200 OK\r\n\r\n", contents);
            stream.write(response.as_bytes()).unwrap();
        };
        stream.flush().unwrap();
    }
    println!("end");
}

fn response_object<'a>(parent_res: &mut ResponseParent, metrics_vec: Vec<String>, counter: &usize) {

    fn shape_vec(str_vec: &String) -> Vec<&str> {
        str_vec.split("  ").into_iter()
            .filter(|x| x.len() > 0)
            .map(|x| x.trim())
            .collect()
    }

    if parent_res.datasets.len() > 0 {
        parent_res.datasets.iter_mut()
            .for_each(|i| i.init(None, None));
        if counter % THRESHOLD_ELEMENT == 0 {
            parent_res.datasets.iter_mut().for_each(|i| i.up_none());
            // parent_res.up();
        }
    }

    let header_vec: Vec<_> = shape_vec(&metrics_vec[0]);
    for data_string in &metrics_vec[1..] {
        let data_vec: Vec<_> = shape_vec(data_string);
        let metrics_data_map: HashMap<_, _> = header_vec.iter()
            .zip(data_vec.iter()).collect();

        let cpu: Option<(_, _)> =  metrics_data_map.get_key_value(&&"CPU %");
        let (_, cpu_v) = cpu.unwrap_or((&&"", &&"0"));
        let cpu_value: &str = &cpu_v[..cpu_v.len()-1];

        let mem: Option<(_, _)> =  metrics_data_map.get_key_value(&&"MEM USAGE / LIMIT");
        let (mem_k, mem_v) = mem.unwrap_or((&&"", &&""));

        let mem_key_vec: Vec<&str> = mem_k.split("/").into_iter().map(|x| x.trim()).collect();
        let mem_value_vec: Vec<&str> = mem_v.split("/").into_iter().map(|x| x.trim()).collect();

        let val = mem_value_vec.iter().any(|&m|  UNIT.iter().any(|&k| m.contains(&k)));
        if !val {
            continue;
        }

        let mem_usage = format!("{}_{}", mem_key_vec[0].to_owned() , &mem_value_vec[0][&mem_value_vec[0].len()-3..]);
        let mem_limit = format!("{}({})", &mem_key_vec[1], &mem_value_vec[1][&mem_value_vec[1].len()-3..]);
        let mem_usage_value = format!("{}", &mem_value_vec[0][..&mem_value_vec[0].len()-3]);
        let mem_limit_value = format!("{}", &mem_value_vec[1][..&mem_value_vec[1].len()-3]);
        let id  = metrics_data_map.get(&&"CONTAINER ID").unwrap_or(&&"").to_string();
        let name  = metrics_data_map.get(&&"NAME").unwrap_or(&&"").to_string();

        fn divide_mem(str: &String, use_val: &String) -> f64 {
            let a= str.split("_").into_iter().collect::<Vec<&str>>();
            match a[1] {
                "KiB" => parse_f(use_val) / 1000.0,
                "MiB" => parse_f(use_val),
                "GiB" => parse_f(use_val) * 1000.0,
                _ => 0.0
            }
        }

        fn parse_f(use_val: &String) -> f64 {
            use_val.parse::<f64>().unwrap()
        }

        if parent_res.limit == None {
            parent_res.limit = Some(format!("{} / {}", mem_limit, mem_limit_value));
        }

        if parent_res.datasets.iter().any(|i| &i.id == &id) {
            parent_res.datasets.iter_mut()
                .filter(|i| &i.id == &id)
                .for_each(|child| {
                    let mem = divide_mem(&mem_usage, &mem_usage_value);
                    let cpu_f = parse_f(&cpu_value.to_string());
                    child.up_val(Some(mem), Some(cpu_f), &counter);
                }
                );
        } else {
            let color = unsafe { line_color().to_string()};
            let mut child = ResponseChild::new(id, name, color);
            let mem = divide_mem(&mem_usage, &mem_usage_value);
            let cpu_f = parse_f(&cpu_value.to_string());
            child.init(Some(mem), Some(cpu_f));
            parent_res.datasets.push_back(child);
        }
    }
}

fn header_tuple<'a>() -> (&'a str, &'a str) {
    ("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-length: ",
     "\r\nServer: metricSrv/99\r\n\r\n")
}
