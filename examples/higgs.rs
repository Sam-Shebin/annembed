//! test of embedding for HIGGS boson data that consists in 11 millions of points in dimension 21 or 28 if we use
//! also the variables hand crafted by physicists.    
//! The data is described and can be retrieved at <https://archive.ics.uci.edu/ml/datasets/HIGGS>.
//! An example of this data set processing is given in the paper by Amid and Warmuth
//! Cf <https://arxiv.org/abs/1910.00204>
//!
//! We run quality estimation with a subsampling factor of 0.15 so we keep about 1_600_000 pointq
//!
//! - With embedding **dimension 2** and embedding neighbourhood size 100 we get :
//! ```text
//! a guess at quality
//! neighbourhood size used in embedding : 6
//! nb neighbourhoods without a match : 925763,  mean number of neighbours conserved when match : 5.136e0
//! embedded radii quantiles at 0.05 : 1.09e-2 , 0.25 : 1.41e-2, 0.5 :  2.30e-2, 0.75 : 1.11e-1, 0.85 : 1.62e-1, 0.95 : 2.62e-1
//!
//! statistics on conservation of neighborhood (of size nbng)
//! neighbourhood size used in target space : 100
//! quantiles on ratio : distance in embedded space of neighbours of origin space / distance of last neighbour in embedded space
//! quantiles at 0.05 : 9.88e-2 , 0.25 : 5.09e-1, 0.5 :  1.72e0, 0.75 : 4.77e0, 0.85 : 7.78e0, 0.95 : 1.57e1
//! ```
//!
//! Only 43% of points have some neighbours conserved and 50% of points need at 1.7 * the radius of embedded neighbour considered to retrieve all
//! their neighbours.
//!
//! - With embedding **dimension 15** and embedding neighbourhood size 100 we get :
//! ```text
//! a guess at quality
//!  neighbourhood size used in embedding : 6
//!  nb neighbourhoods without a match : 104259,  mean number of neighbours conserved when match : 5.783e0
//!  embedded radii quantiles at 0.05 : 7.73e-2 , 0.25 : 1.14e-1, 0.5 :  2.20e-1, 0.75 : 5.49e-1, 0.85 : 6.28e-1, 0.95 : 7.52e-1
//!
//! statistics on conservation of neighborhood (of size nbng)
//! neighbourhood size used in target space : 100
//! quantiles on ratio : distance in embedded space of neighbours of origin space / distance of last neighbour in embedded space
//! quantiles at 0.05 : 4.43e-2 , 0.25 : 1.11e-1, 0.5 :  2.49e-1, 0.75 : 5.50e-1, 0.85 : 7.70e-1, 0.95 : 1.27e0
//! ```
//! So over 1_600_000 nodes only 100_000 do not retrieve their neighbours. 85% of points have their neighbours retrieved within 0.8 * the radius of
//! embedded neighbour considered.
//!
//!
use annembed::diffmaps::DiffusionParams;
use anyhow::anyhow;

use clap::{Arg, ArgAction, ArgMatches, Command};

use std::fs::OpenOptions;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use rand::distributions::{Distribution, Uniform};

use csv::Writer;

use ndarray::{Array2, ArrayView};

use hnsw_rs::prelude::*;

use annembed::prelude::*;

use cpu_time::ProcessTime;
use std::time::{Duration, SystemTime};

use annembed::diffmaps::*;
use annembed::fromhnsw::kgproj::KGraphProjection;
use annembed::fromhnsw::kgraph::{kgraph_from_hnsw_all, kgraph_from_hnsw_layer};

const HIGGS_DIR: &str = "/home/jpboth/Data/";

/// return a vector of labels, and a list of vectors to embed
/// First field of record is label, then the 21 following field are the data.
/// 11 millions records!
fn read_higgs_csv(
    fname: String,
    nb_column: usize,
    // subsampling factor
    subsampling_factor: f64,
) -> anyhow::Result<(Vec<u8>, Array2<f32>)> {
    //
    let nb_fields = 29;
    let to_parse = nb_column;
    let nb_var = nb_column - 1;
    let mut num_record: usize = 0;
    let filepath = PathBuf::from(fname);
    let fileres = OpenOptions::new().read(true).open(&filepath);
    if fileres.is_err() {
        log::error!("read_higgs_csv {:?}", filepath.as_os_str());
        println!("read_higgs_csv {:?}", filepath.as_os_str());
        return Err(anyhow!(
            "directed_from_csv could not open file {}",
            filepath.display()
        ));
    }
    let file = fileres?;
    let bufreader = BufReader::new(file);
    let mut labels = Vec::<u8>::new();
    let mut data = Array2::<f32>::zeros((0, nb_var));
    let mut rdr = csv::Reader::from_reader(bufreader);
    //
    let unif_01 = Uniform::<f64>::new(0., 1.);
    let mut rng = rand::thread_rng();
    //
    for result in rdr.records() {
        // The iterator yields Result<StringRecord, Error>, so we check the
        // error here.
        num_record += 1;
        // sample if we load this record
        let xsi = unif_01.sample(&mut rng);
        if xsi >= subsampling_factor {
            continue;
        }
        //
        if num_record % 1_000_000 == 0 {
            log::info!("read {} record", num_record);
        }
        let record = result?;
        if record.len() != nb_fields {
            println!("record {} record has {} fields", num_record, record.len());
            return Err(anyhow!(
                "record {} record has {} fields",
                num_record,
                record.len()
            ));
        }
        let mut new_data = Vec::<f32>::with_capacity(21);
        for j in 0..to_parse {
            let field = record.get(j).unwrap();
            // decode into Ix type
            if let Ok(val) = field.parse::<f32>() {
                match j {
                    0 => {
                        labels.push(if val > 0. { 1 } else { 0 });
                    }
                    _ => {
                        new_data.push(val);
                    }
                };
            } else {
                log::debug!("error decoding field  of record {}", num_record);
                return Err(anyhow!("error decoding field 1of record  {}", num_record));
            }
        } // end for j
        assert_eq!(new_data.len(), nb_var);
        data.push_row(ArrayView::from(&new_data)).unwrap();
    }
    //
    assert_eq!(data.dim().0, labels.len());
    log::info!("number of records loaded : {:?}", data.dim().0);
    //
    Ok((labels, data))
} // end of read_higgs_csv

// refromat and possibly rescale
fn reformat(data: &mut Array2<f32>, rescale: bool) -> Vec<Vec<f32>> {
    let (nb_row, nb_col) = data.dim();
    let mut datavec = Vec::<Vec<f32>>::with_capacity(nb_row);
    //
    if rescale {
        for j in 0..nb_col {
            let mut col = data.column_mut(j);
            let mean = col.mean().unwrap();
            let sigma = col.var(1.).sqrt();
            col.mapv_inplace(|x| (x - mean) / sigma);
        }
    }
    // reformat in vetors
    for i in 0..nb_row {
        datavec.push(data.row(i).to_vec());
    }
    //
    datavec
} // end of reformat

//==================================================================

struct HiggsArg {
    dmap: bool,
    sampling_factor: f64,
}

// returns true if dmap is asked
fn parse_higgs_matches(argmatches: &ArgMatches) -> HiggsArg {
    let sampling_factor = *argmatches.get_one::<f64>("subsampling").unwrap();
    // for now just that
    let dmap = argmatches.contains_id("dmap");
    log::info!("diffusion map asked");
    HiggsArg {
        dmap: dmap,
        sampling_factor,
    }
} // end of parse_higgs_matches

//

// do umap like embeddding
fn entropy_embedding(labels: &Vec<u8>, hnsw: &Hnsw<f32, DistL2>, sampling_factor: f64) {
    //
    log::info!("doing umap like embedding");
    //
    let mut embed_params = EmbedderParams::default();
    embed_params.nb_grad_batch = 25;
    embed_params.scale_rho = 0.75;
    embed_params.beta = 1.;
    embed_params.grad_step = 1.;
    embed_params.nb_sampling_by_edge = 10;
    embed_params.dmap_init = true;
    // embed_params.asked_dim = 15;
    //
    // if set to true, we use first layer embedding to initialize the whole embedding
    // ==============================================================================
    let hierarchical = true;
    //===============================================================================
    let mut embedder;
    let kgraph;
    let graphprojection;
    if !hierarchical {
        let knbn = 6;
        log::info!("embedding from neighbourhood of size : {}", knbn);
        kgraph = kgraph_from_hnsw_all(&hnsw, knbn).unwrap();
        embedder = Embedder::new(&kgraph, embed_params);
        let embed_res = embedder.embed();
        assert!(embed_res.is_ok());
        assert!(embedder.get_embedded().is_some());
    } else {
        let knbn = 6;
        log::info!("embedding from neighbourhood of size : {}", knbn);
        let projection_layer = 1;
        embed_params.nb_grad_batch = 40;
        embed_params.grad_factor = 5;
        log::info!("graph projection on layer : {}", projection_layer);
        graphprojection = KGraphProjection::<f32>::new(&hnsw, knbn, projection_layer);
        embedder = Embedder::from_hkgraph(&graphprojection, embed_params);
        let embed_res = embedder.embed();
        assert!(embed_res.is_ok());
        assert!(embedder.get_embedded().is_some());
    }

    // dump
    log::info!("dumping initial embedding in csv file");
    let mut csv_w = Writer::from_path("higgs_initial_embedded.csv").unwrap();
    // we can use get_embedded_reindexed as we indexed DataId contiguously in hnsw!
    let _res = write_csv_labeled_array2(
        &mut csv_w,
        labels.as_slice(),
        &embedder.get_initial_embedding_reindexed(),
    );
    csv_w.flush().unwrap();

    log::info!("dumping in csv file");
    let mut csv_w = Writer::from_path("higgs_embedded.csv").unwrap();
    // we can use get_embedded_reindexed as we indexed DataId contiguously in hnsw!
    let _res = write_csv_labeled_array2(
        &mut csv_w,
        labels.as_slice(),
        &embedder.get_embedded_reindexed(),
    );
    csv_w.flush().unwrap();
    //
    //
    // quality too memory consuming. must subsample
    //
    if sampling_factor <= 0.2 {
        log::info!("estimating quality");
        let _quality = embedder.get_quality_estimate_from_edge_length(100);
    }
}

// diffusionmap embedding
fn dmap_embedding(
    labels: &Vec<u8>,
    hnsw: &Hnsw<f32, DistL2>,
    layer: usize,
    dmap_params: &DiffusionParams,
) {
    log::info!(
        "\n doing dmap embedding, hnsw total nb points : {} using layers from layer {}",
        hnsw.get_nb_point(),
        layer
    );
    let embedded: Array2<f32>;
    //
    let mut dmapembedder = DiffusionMaps::new(*dmap_params);
    if layer >= 1 {
        let kgraph = kgraph_from_hnsw_layer::<f32, DistL2, f32>(hnsw, 8, 1).unwrap();
        let res = dmapembedder.embed_from_kgraph(&kgraph, dmap_params);
        if res.is_err() {
            log::error!("dmap_embedding failed");
        }
        embedded = res.unwrap();
    } else {
        let res = dmapembedder.embed_from_hnsw::<f32, DistL2, f32>(hnsw, dmap_params);
        if res.is_err() {
            log::error!("dmap_embedding failed");
        }
        embedded = res.unwrap();
    }
    log::info!("dumping in csv file");
    let mut csv_w = Writer::from_path("higgs_dmap_embedded.csv").unwrap();
    // we can use get_embedded_reindexed as we indexed DataId contiguously in hnsw!
    let _res = write_csv_labeled_array2(&mut csv_w, labels.as_slice(), &embedded);
    csv_w.flush().unwrap();
} // end of dmap_embedding

//====================================================================================

///  By defaut a umap like embedding is done.
///  The command takes the following args:
///
///  * --dmap is used it is a dmap embedding
///   
///  * --factor sampling_factor
///       sampling_factor : if >= 1. full data is embedded, but quality runs only with 64Gb for sampling_factor <= 0.15  
///  * --dist "DistL2" or "DistL1"
///
///  The others variables can be modified in the code
///
///  - nb_col          : number of columns to read, 22 or 29  
///  - rescale         : true, can be set to false to check possible effect (really tiny)  
///  - hierarchical    : if true use first layer to initialize embedding  
///  - knbn            : to use various neighbourhood size for edge sampling  
///  - asked_dim       : default to 2 but in conjuntion with sampling factor, can see the impact on quality.  
pub fn main() {
    //
    let _ = env_logger::builder().is_test(true).try_init();
    //
    let higgcmdarg = Command::new("higgs")
    .arg(Arg::new("dist")
        .long("dist")
        .short('d')
        .required(true)
        .action(ArgAction::Set)
        .value_parser(clap::value_parser!(String))
        .help("distance is required   \"DistL1\" , \"DistL2\", \"DistCosine\", \"DistJeyffreys\"  "))
    .arg(Arg::new("subsampling")
        .long("factor")
        .required(false)
        .action(ArgAction::Set)
        .value_parser(clap::value_parser!(f64))
        .default_value("1.0")
        .help("subsampling factor between 0. and 1."))
    .arg(Arg::new("dmap")
        .long("dmap")
        .required(false)
        .action(ArgAction::SetTrue)
        .help("takes no value")).get_matches();
    //
    let higgsarg = parse_higgs_matches(&higgcmdarg);
    //
    let mut fname = String::from(HIGGS_DIR);
    // parameters to play with
    // choose if we run on 22 or 29 columns id estimation on 21 or 28 variables
    // first column is label. We have one column more than variables
    //====================
    let nb_col = 29;
    let rescale = true;
    // quality estimation requires subsampling factor of 0.15 is Ok with 64Gb
    let sampling_factor = higgsarg.sampling_factor;
    //====================
    let nb_var = nb_col - 1;
    //
    fname.push_str("HIGGS.csv");
    log::info!("using subsampling factor : {:?}", sampling_factor);
    let res = read_higgs_csv(fname, nb_col, sampling_factor);
    if res.is_err() {
        log::error!("error reading Higgs.csv {:?}", &res.as_ref().err().as_ref());
        std::process::exit(1);
    }
    let mut res = res.unwrap();
    let labels = res.0;
    // =====================
    let data = reformat(&mut res.1, rescale);
    drop(res.1); // we do not need res.1 anymore
    assert_eq!(data.len(), labels.len());
    let cpu_start = ProcessTime::now();
    // DO we have a dump ?
    let sys_now = SystemTime::now();
    let cpu_time = ProcessTime::now();
    // The following will try to reload hnsw from files Higgs.hnsw.data and Results/Higgs.hnsw.graph
    // supposed to be in current directory.
    let directory = PathBuf::from(".");
    // reloader must be declared before hnsw as it holds references used in hnsw
    // and varibles are dropped in reverse order of declaration!
    let varstring: String = nb_var.to_string();
    let mut basename = String::from("Higgs-");
    basename.push_str(&varstring);
    let mut reloader = HnswIo::new(&directory, &basename);
    let mut hnsw_opt: Option<Hnsw<f32, DistL2>> = None;
    let mut hnsw: Hnsw<f32, DistL2>;
    //
    // if we do not sub sample we try reloading
    if sampling_factor >= 1. {
        let res_reload = reloader.load_hnsw::<f32, DistL2>();
        println!(
            " higgs ann reload sys time(s) {:?} cpu time {:?}",
            sys_now.elapsed().unwrap().as_secs(),
            cpu_time.elapsed().as_secs()
        );
        if res_reload.is_ok() {
            hnsw_opt = Some(res_reload.unwrap());
        }
    }
    //
    if hnsw_opt.is_some() {
        hnsw = hnsw_opt.unwrap();
        hnsw.dump_layer_info();
        let cpu_time: Duration = cpu_start.elapsed();
        println!(
            " higgs ann reload sys time(s) {:?} cpu time {:?}",
            sys_now.elapsed().unwrap().as_secs(),
            cpu_time.as_secs()
        );
        drop(data);
    } else {
        // need to construct hnsw
        log::info!("no Hnsw dump found in directory, reconstructing Hnsw structure");
        //
        let cpu_start_hnsw = ProcessTime::now();
        let sys_start_hnsw = SystemTime::now();
        //
        let ef_c = 400;
        let max_nb_connection = 24;
        let nbdata = data.len();
        let nb_layer = 16.min((nbdata as f32).ln().trunc() as usize);
        //
        hnsw = Hnsw::<f32, DistL2>::new(max_nb_connection, nbdata, nb_layer, ef_c, DistL2 {});
        hnsw.set_keeping_pruned(true);
        hnsw.modify_level_scale(0.5);
        // we insert by block of 1_000_000
        let block_size = 1_000_000;
        let mut inserted = 0;
        let mut numblock = 0;
        while inserted < data.len() {
            let block_length = ((numblock + 1) * block_size).min(nbdata) - numblock * block_size;
            let mut data_with_id = Vec::<(&[f32], usize)>::with_capacity(block_length);
            for _ in 0..block_length {
                data_with_id.push((&data[inserted], inserted));
                inserted += 1;
            }
            hnsw.parallel_insert_slice(&data_with_id);
            numblock += 1;
        }
        // images as vectors of f32 and send to hnsw
        println!(
            " higgs ann construction sys time(s) {:?} cpu time {:?}",
            sys_start_hnsw.elapsed().unwrap().as_secs(),
            cpu_start_hnsw.elapsed().as_secs()
        );
        hnsw.dump_layer_info();
        if sampling_factor >= 1. {
            // if we did not subsample we save hnsw to avoid reconstruction runs in 0.4 hour...on my laptop
            // We dump in Higgs-$nb_var.hnsw.data and Higgs-$nb_var.hnsw.graph
            let mut fname = String::from("Higgs");
            fname.push('-');
            fname.push_str(&varstring);
            let path = Path::new("./");
            let _res = hnsw.file_dump(path, &fname);
        }
    }
    //
    // now we embed
    //
    let sys_now = SystemTime::now();
    let cpu_start = ProcessTime::now();
    //
    let dmap = higgsarg.dmap;
    if !dmap {
        entropy_embedding(&labels, &hnsw, sampling_factor);
    } else {
        let mut dmap_params = DiffusionParams::default();
        dmap_params.set_embedding_dimension(5);
        dmap_params.set_alfa(1.);
        dmap_params.set_beta(0.);
        if sampling_factor >= 0.5 {
            // embed from layer 1 upper to spare memory if full dat are loaded
            dmap_embedding(&labels, &hnsw, 1, &dmap_params);
        } else {
            // embed from layer 1 upper to spare memory if full dat are loaded
            dmap_embedding(&labels, &hnsw, 0, &dmap_params);
        }
    }
    println!(
        " ann embed total sys time(s) {:.2e}  cpu time {:.2e}",
        sys_now.elapsed().unwrap().as_secs(),
        cpu_start.elapsed().as_secs()
    );
} // end of main
