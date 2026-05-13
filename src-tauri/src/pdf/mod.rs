pub mod form_generator;
pub mod comparison_generator;
pub mod comparison_html_generator;
pub mod comparison_image_pdf_generator;
pub mod form_builder;
pub mod form_html_generator;
pub mod image_embedder;
pub mod invoice_pipeline;
pub mod text_extractor;

pub use form_builder::build_reimbursement_form;
pub use image_embedder::{render_pdf_page_to_png, render_pdf_all_pages_to_pngs, is_supported_image};
pub use invoice_pipeline::{
    parse_invoice_from_pdf, parse_invoice_from_image, parse_itinerary_from_pdf,
    parse_all_from_dir, pair_invoices_with_itineraries, ItineraryDoc, ParseResult,
};
