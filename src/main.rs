use std::fs::File;
use tiff::decoder::Decoder;
use tiff::encoder::TiffEncoder;
use tiff::encoder::colortype;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use ordered_float::OrderedFloat;
use std::sync::Arc;
use std::thread;
use std::time::{Instant};
use std::sync::mpsc;
// This makes the csv crate accessible to your program.
extern crate csv;
// Import the standard library's I/O module so we can read from stdin.
//use std::io;

#[derive(Copy, Clone, Eq, PartialEq,Debug)]
struct Cell {
        row : usize,
        col : usize,
        friccion: OrderedFloat<f32>,
        key: u32,
    }    
impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {//regresa menor friccion
        other.friccion.cmp(&self.friccion)//compara izquierda con derecha 
            .then_with(|| other.key.cmp(&self.key))//si te da eq la comparacion de friccion
            //funcion tipo Ordering
            //izquierda<derecha = lt
            //derecha<izquierda =gt
            //derecha=izquierda eq
    }
}

impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
fn main() {
    let inicio = Instant::now();
    let mut rows=0;
    let mut cols=0;
    let fric_img = match leer_img("/root/modelos/Kenia/fricc_singeo0.tif",&mut rows,&mut cols){
        tiff::decoder::DecodingResult::F32(v) => v,
        _ => panic!("paniccccc"),
    };
    let locs_img = match leer_img("/root/modelos/Kenia/20_comunidades/mapa_locs_20.tif",&mut rows,&mut cols){
        tiff::decoder::DecodingResult::F32(v) => v,
        _ => panic!("paniccccc "),
    };
    let mut localidades = leer_csv("/root/modelos/Kenia/20_comunidades/fwuse_20.csv");
    for row in 0..rows{
        for col in 0..cols{
            if locs_img[(cols*row)+col]!=-9999.0{
                if let Some(elem) = localidades.get_mut(locs_img[(cols*row)+col] as usize-1){ 
                    elem.row=row;
                    elem.col=col;
                }
            }
        }
    } 
    let mut idw_final = Vec::new();
    idw_final.resize(fric_img.len(),0f32);

    let calculo = Instant::now();
    let localidades2=localidades.clone();
    let fric_img=Arc::new(fric_img);

    let (tx,rx)=mpsc::channel();

    while let Some(com) = localidades.pop(){
        let fric_img = Arc::clone(&fric_img);
        let tx1=tx.clone();
        thread::spawn(move ||{   
            let cd_matrix=cd_met(&com,rows, cols,&fric_img);    
            let idw_parcial=idw_met(&com,&cd_matrix);  
            tx1.send(idw_parcial).unwrap();
        });
    }
    drop(tx);
    for idw_parcial in rx{
        for a in 0..idw_final.len(){
            idw_final[a]+=idw_parcial[a];
        }
    }
    
    println!("tiempo calculo: {}", calculo.elapsed().as_millis());
    for com in localidades2.iter() {
        let it =((cols*com.row)+com.col) as usize;
        idw_final[it]=-9999.0;
    }
    for row in 0..rows{
        for col in 0..cols{
            if idw_final[(cols*row)+col]<=0.0{
                idw_final[(cols*row)+col]=-9999.0; 
            }
        }
    }
    let archivo= File::create("/root/modelos/Kenia/20_comunidades/ParallelR/IDW_20Rust.tif").unwrap();
    let mut image =TiffEncoder::new(archivo).unwrap();
    image.write_image::<colortype::Gray32Float>(cols as u32,rows as u32 , &idw_final).unwrap();
    println!("tiempo total: {}", inicio.elapsed().as_secs());
    comparar_tif("/root/modelos/Kenia/20_comunidades/D5/IDW_20.tif", "/root/modelos/Kenia/20_comunidades/ParallelR/IDW_20Rust.tif");
    
}
//--------------------------------------------------------
//--------------------------------------------------------    
fn leer_csv(nombre:&str) -> Vec<Cell>{
    let mut localidades:Vec<Cell>=Vec::new();
    let mut rdr = csv::Reader::from_path(nombre).expect("no leido");
    for result in rdr.records() {
        let record = result.expect("a CSV record");
        let loc =record.get(0).expect("leido");
        let localidad = loc.parse::<u32>().unwrap();
        let req =record.get(1).expect("leido");
        let requisitos = req.parse::<f32>().unwrap();
        let tmp:Cell=Cell{
            row:0,
            col:0,
            friccion:OrderedFloat(requisitos),
            key:localidad,
        };
        localidades.push(tmp);
    }
    return localidades;
}
fn leer_img(nombre:&str,rows: &mut usize,cols: &mut usize) -> tiff::decoder::DecodingResult {   
    let contents = File::open(nombre).unwrap();
    let mut preimage = Decoder::new(contents).unwrap();
    let (cols_i,rows_i)=preimage.dimensions().unwrap();
    let image = preimage.read_image().unwrap();
    *rows=rows_i as usize;
    *cols=cols_i as usize;
    return image;
}
fn cd_met(comunidad: &Cell, rows: usize, cols: usize, fric_matrix: &Vec<f32>) -> Vec<f32>{
    let rows=rows as isize;
    let cols=cols as isize;
    let mut cd_matrix = Vec::new();
    cd_matrix.resize(fric_matrix.len(),f32::INFINITY);
    let mut cd_map = BinaryHeap::new();   
    let mut pos_cell=comunidad.clone();
    pos_cell.key=0;
    pos_cell.friccion=OrderedFloat(0.0);
    cd_map.push(pos_cell.clone());//comunidad inicial
    let mut cont = 1;
    let mut row_temp;
    let mut col_temp;
    let mov: [[isize;8];2] =[[1,1,0,-1,-1,-1,0,1],[0,1,1,1,0,-1,-1,-1]];
    //---------------------------------------------------------------inicia calculo
    while let Some(Cell {row,col, friccion:costo_acumulado, key:_ }) = cd_map.pop(){
       // println!("{}",key);
        for i in 1..9{
            let mut pos_cell=pos_cell.clone();
            row_temp = mov[1][i - 1] + row as isize;
            col_temp = mov[0][i - 1] + col as isize;
            let it =((cols*row_temp)+col_temp) as usize;
            if row_temp < rows && row_temp >= 0 && col_temp < cols && col_temp >= 0 {
                if fric_matrix[((cols*row_temp)+col_temp) as usize] > 0.0 {
                    if i % 2 != 0{//si es un movimiento lateral
                        pos_cell.friccion = costo_acumulado + OrderedFloat(fric_matrix[it]);
                    }
                    else{//si es un movimiento diagonal
                        pos_cell.friccion = costo_acumulado + OrderedFloat(fric_matrix[it] * 2f32.sqrt());
                    }
                    //se busca el menor valor de CD, es posible que se escriba varias veces en una celda
                    if OrderedFloat(cd_matrix[it]) > pos_cell.friccion {
                        pos_cell.row = row_temp as usize;
                        pos_cell.col = col_temp as usize;
                        pos_cell.key=cont;
                        cont+=1;
                        let OrderedFloat(fric_mov) = pos_cell.friccion;
                        cd_matrix[it] = fric_mov;
                        cd_map.push(pos_cell);
                    }
                }
            }
        }
    }
    return cd_matrix;
}
fn idw_met (comunidad: &Cell,cd_matrix: &Vec<f32>)->Vec<f32>{
    let mut idw_matrix = Vec::new();
    idw_matrix.resize(cd_matrix.len(),0f32);
    let exp=1.005;
    let OrderedFloat(req)=comunidad.friccion; 
    for i in 0..idw_matrix.len(){
        if cd_matrix[i]<=0.0{
            idw_matrix[i]=-9999.0;
        }
        else{
            idw_matrix[i] += req / cd_matrix[i].powf(exp);
        }
    }
    return idw_matrix;
}
fn comparar_tif(img1:&str, img2:&str){
    let mut rows=0;
    let mut cols=0; 
    let mut val_abs;
    let mut distintas=0;
    let mat1 = match leer_img(img1,&mut rows,&mut cols){
        tiff::decoder::DecodingResult::F32(v) => v,
        _ => panic!("paniccccc"),
    };
    println!("imagen 1, rows: {} cols: {}",rows,cols);
    let mat2 = match leer_img(img2,&mut rows,&mut cols){
        tiff::decoder::DecodingResult::F32(v) => v,
        _ => panic!("paniccccc"),
    };
    println!("imagen 2, rows: {} cols: {}",rows,cols);
    for i in 0..mat2.len(){
        val_abs=(mat1[i]-mat2[i]).abs();
        val_abs/=mat1[i];//error relativo 
        if val_abs > 0.001{
            distintas+=1;
            println!("D5: {}, Rust: {}",mat1[i],mat2[i]);
        }    
    }
    println!("celdas distintas: {}",distintas);
}
